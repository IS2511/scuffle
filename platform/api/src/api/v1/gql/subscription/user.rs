use async_graphql::{Context, SimpleObject, Subscription};
use futures_util::Stream;
use prost::Message;
use ulid::Ulid;
use uuid::Uuid;

use crate::api::v1::gql::{
    error::{GqlError, Result, ResultExt},
    ext::ContextExt,
    models::{color::DisplayColor, ulid::GqlUlid},
};

use super::FollowStream;

#[derive(Default)]
pub struct UserSubscription;

#[derive(SimpleObject)]
struct DisplayNameStream {
    pub user_id: GqlUlid,
    pub display_name: String,
}

#[derive(SimpleObject)]
struct DisplayColorStream {
    pub user_id: GqlUlid,
    pub display_color: DisplayColor,
}

#[Subscription]
impl UserSubscription {
    async fn user_display_name<'ctx>(
        &self,
        ctx: &'ctx Context<'ctx>,
        user_id: GqlUlid,
    ) -> Result<impl Stream<Item = Result<DisplayNameStream>> + 'ctx> {
        let global = ctx.get_global();

        let Some(display_name) = global
            .user_by_id_loader
            .load_one(user_id.into())
            .await
            .map_err_gql("Failed to fetch user")?
            .map(|u| u.display_name)
        else {
            return Err(GqlError::NotFound
                .with_message("user not found")
                .with_field(vec!["user_id"]));
        };

        let mut subscription = global
            .subscription_manager
            .subscribe(format!("user.{}.display_name", *user_id))
            .await
            .map_err_gql("Failed to subscribe to user display name")?;

        Ok(async_stream::stream!({
            yield Ok(DisplayNameStream {
                user_id,
                display_name,
            });

            while let Ok(message) = subscription.recv().await {
                let event = pb::scuffle::platform::internal::events::UserDisplayName::decode(
                    message.payload,
                )
                .map_err_gql("Failed to decode user display name")?;

                let user_id = Ulid::from_string(&event.user_id)
                    .map_err_gql("Failed to decode user id")?
                    .into();

                yield Ok(DisplayNameStream {
                    user_id,
                    display_name: event.display_name,
                });
            }
        }))
    }

    async fn user_display_color<'ctx>(
        &self,
        ctx: &'ctx Context<'ctx>,
        user_id: GqlUlid,
    ) -> Result<impl Stream<Item = Result<DisplayColorStream>> + 'ctx> {
        let global = ctx.get_global();

        let Some(display_color) = global
            .user_by_id_loader
            .load_one(user_id.into())
            .await
            .map_err_gql("Failed to fetch user")?
            .map(|u| u.display_color)
        else {
            return Err(GqlError::NotFound
                .with_message("user not found")
                .with_field(vec!["user_id"]));
        };

        let mut subscription = global
            .subscription_manager
            .subscribe(format!("user.{}.display_color", *user_id))
            .await
            .map_err_gql("Failed to subscribe to user display name")?;

        Ok(async_stream::stream!({
            yield Ok(DisplayColorStream {
                user_id,
                display_color: display_color.into(),
            });

            while let Ok(message) = subscription.recv().await {
                let event = pb::scuffle::platform::internal::events::UserDisplayColor::decode(
                    message.payload,
                )
                .map_err_gql("Failed to decode user display name")?;

                let user_id = Ulid::from_string(&event.user_id)
                    .map_err_gql("Failed to decode user id")?
                    .into();

                yield Ok(DisplayColorStream {
                    user_id,
                    display_color: event.display_color.into(),
                });
            }
        }))
    }

    async fn user_following<'ctx>(
        &self,
        ctx: &'ctx Context<'ctx>,
        #[graphql(desc = "When specified, this subscription is limited to only this channel.")]
        channel_id: Option<GqlUlid>,
    ) -> Result<impl Stream<Item = Result<FollowStream>> + 'ctx> {
        let global = ctx.get_global();
        let request_context = ctx.get_req_context();

        let auth = request_context
            .auth()
            .await
            .ok_or(GqlError::Unauthorized.with_message("You need to be logged in"))?;

        let user_id: Ulid = auth.session.user_id.into();

        let mut subscription = global
            .subscription_manager
            .subscribe(format!("user.{}.follows", user_id.to_string()))
            .await
            .map_err_gql("Failed to subscribe to user follows")?;

        Ok(async_stream::stream!({
            if let Some(channel_id) = channel_id {
                let (is_following,): (bool,) = sqlx::query_as(
                    "SELECT following FROM channel_user WHERE user_id = $1 AND channel_id = $2",
                )
                .bind(auth.session.user_id)
                .bind(Uuid::from(channel_id))
                .fetch_optional(&*global.db)
                .await
                .map_err_gql("Failed to fetch channel_user")?
                .unwrap_or((false,));
                yield Ok(FollowStream {
                    user_id: user_id.into(),
                    channel_id,
                    following: is_following,
                });
            }

            while let Ok(message) = subscription.recv().await {
                let event = pb::scuffle::platform::internal::events::UserFollowChannel::decode(
                    message.payload,
                )
                .map_err_gql("Failed to decode user follow")?;

                let user_id = Ulid::from_string(&event.user_id)
                    .map_err_gql("Failed to decode user id")?
                    .into();
                let event_channel_id = Ulid::from_string(&event.channel_id)
                    .map_err_gql("Failed to decode channel id")?;

                if channel_id.is_some_and(|i| event_channel_id != *i) {
                    continue;
                }

                yield Ok(FollowStream {
                    user_id,
                    channel_id: event_channel_id.into(),
                    following: event.following,
                });
            }
        }))
    }
}