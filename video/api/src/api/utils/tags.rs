use std::collections::HashMap;

use pb::scuffle::video::v1::types::Tags;
use tonic::Status;
use ulid::Ulid;
use uuid::Uuid;
use video_common::database::DatabaseTable;

const MAX_TAG_COUNT: usize = 10;
const MAX_TAG_KEY_LENGTH: usize = 16;
const MAX_TAG_VALUE_LENGTH: usize = 32;
const TAG_ALPHABET: &str = r#"abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789-+?'";:[]{}_"#;

pub fn validate_tags(tags: Option<&Tags>) -> tonic::Result<()> {
	if let Some(tags) = tags {
		if tags.tags.len() > MAX_TAG_COUNT {
			return Err(Status::invalid_argument(format!("too many tags, max {MAX_TAG_COUNT}")));
		}

		for (key, value) in &tags.tags {
			if key.len() > MAX_TAG_KEY_LENGTH {
				return Err(Status::invalid_argument(format!(
					"tag key too long, max length {MAX_TAG_KEY_LENGTH} characters"
				)));
			}

			if value.len() > MAX_TAG_VALUE_LENGTH {
				return Err(Status::invalid_argument(format!(
					"tag value too long, max length {MAX_TAG_VALUE_LENGTH} characters",
				)));
			}

			if let Some(c) = key.chars().find(|c| !TAG_ALPHABET.contains(*c)) {
				return Err(Status::invalid_argument(format!(
					"tag key contains invalid characters: {key}: {c}"
				)));
			}

			if let Some(c) = value.chars().find(|c| !TAG_ALPHABET.contains(*c)) {
				return Err(Status::invalid_argument(format!(
					"tag value contains invalid characters: {value}: {c}"
				)));
			}
		}
	}

	Ok(())
}

pub fn validate_tags_array(tags: &[String]) -> tonic::Result<()> {
	if tags.len() > MAX_TAG_COUNT {
		return Err(Status::invalid_argument(format!("too many tags, max {MAX_TAG_COUNT}")));
	}

	for tag in tags {
		if tag.len() > MAX_TAG_KEY_LENGTH {
			return Err(Status::invalid_argument(format!(
				"tag key too long, max length {MAX_TAG_KEY_LENGTH} characters"
			)));
		}

		if !tag.chars().all(|c| TAG_ALPHABET.contains(c)) {
			return Err(Status::invalid_argument("tag key contains invalid characters"));
		}
	}

	Ok(())
}

#[derive(sqlx::FromRow)]
pub struct TagExt {
	pub tags: sqlx::types::Json<HashMap<String, String>>,
	pub status: i64,
}

impl TagExt {
	pub fn into_tags(self) -> tonic::Result<Tags> {
		match self.status {
			0 | 1 => {}
			2 => {
				return Err(Status::invalid_argument(
					"too many tags, cannot add tag(s) to exceed max tag count",
				));
			}
			_ => {
				return Err(Status::internal(format!(
					"invalid status code returned from query: {}",
					self.status
				)));
			}
		}

		Ok(Tags { tags: self.tags.0 })
	}
}

pub fn add_tag_query<D: DatabaseTable>(
	tags: &HashMap<String, String>,
	id: Ulid,
	organization_id: Option<Ulid>,
) -> sqlx::QueryBuilder<'_, sqlx::Postgres> {
	let mut qb = sqlx::QueryBuilder::default();

	qb.push("WITH mt AS (SELECT id, tags || ")
		.push_bind(sqlx::types::Json(tags))
		.push(" AS new_tags, CASE WHEN tags @> $1 THEN 1 WHEN COUNT(jsonb_object_keys(tags || $1)) > ")
		.push_bind(MAX_TAG_COUNT as i32)
		.push(" THEN 2 ELSE 0 END AS status FROM ")
		.push(D::NAME)
		.push(" WHERE id = ")
		.push_bind(Uuid::from(id));

	if let Some(organization_id) = organization_id {
		qb.push(" AND organization_id = ").push_bind(Uuid::from(organization_id));
	}

	qb.push(" GROUP BY id, organization_id) UPDATE ")
		.push(D::NAME)
		.push(" AS t SET tags = CASE WHEN mt.status = 0 THEN mt.new_tags ELSE tags END,")
		.push(" updated_at = CASE WHEN mt.status = 0 THEN now() ELSE updated_at END")
		.push(" FROM mt WHERE t.id = mt.id RETURNING t.tags as tags, mt.status as status;");

	qb
}

pub fn remove_tag_query<D: DatabaseTable>(
	tags: &[String],
	id: Ulid,
	organization_id: Option<Ulid>,
) -> sqlx::QueryBuilder<'_, sqlx::Postgres> {
	let mut qb = sqlx::QueryBuilder::default();

	qb.push("WITH rt AS (SELECT id, tags - ")
		.push_bind(tags)
		.push(" AS new_tags, CASE WHEN NOT tags ?| $1 THEN 1 ELSE 0 END AS status FROM ")
		.push(D::NAME)
		.push(" WHERE id = ")
		.push_bind(Uuid::from(id));

	if let Some(organization_id) = organization_id {
		qb.push(" AND organization_id = ").push_bind(Uuid::from(organization_id));
	}

	qb.push(" GROUP BY id, organization_id) UPDATE ")
		.push(D::NAME)
		.push(" AS t")
		.push(" SET tags = CASE WHEN rt.status = 0 THEN rt.new_tags ELSE tags END,")
		.push(" updated_at = CASE WHEN rt.status = 0 THEN now() ELSE updated_at END")
		.push(" FROM rt WHERE t.id = rt.id RETURNING t.tags AS tags, rt.status AS status;");

	qb
}

macro_rules! impl_tag_req {
	($req:ty, $resp:ty) => {
		#[async_trait::async_trait]
		impl crate::api::utils::QbRequest for $req {
			type QueryObject = crate::api::utils::tags::TagExt;

			async fn build_query<G: crate::global::ApiGlobal>(
				&self,
				_: &std::sync::Arc<G>,
				access_token: &video_common::database::AccessToken,
			) -> tonic::Result<sqlx::QueryBuilder<'_, sqlx::Postgres>> {
				crate::api::utils::tags::validate_tags(self.tags.as_ref())?;

				let tags = self.tags.as_ref().ok_or_else(|| {
					tonic::Status::invalid_argument("tags must be provided to add a tag")
				})?;

				Ok(crate::api::utils::tags::add_tag_query::<Self::Table>(&tags.tags, pb::ext::UlidExt::into_ulid(self.id), Some(access_token.organization_id.0)))
			}
		}

		impl crate::api::utils::QbResponse for $resp {
			type Request = $req;

			fn from_query_object(query_object: Vec<<Self::Request as crate::api::utils::QbRequest>::QueryObject>) -> tonic::Result<Self> {
				if query_object.len() != 1 {
					return Err(tonic::Status::internal(format!(
						"failed to create {}, {} rows returned",
						<<Self::Request as crate::api::utils::TonicRequest>::Table as video_common::database::DatabaseTable>::FRIENDLY_NAME,
						query_object.len(),
					)));
				}

				Ok(Self {
					tags: Some(query_object.into_iter().next().unwrap().into_tags()?),
				})
			}
		}
	};
}

pub(crate) use impl_tag_req;

macro_rules! impl_untag_req {
	($req:ty, $resp:ty) => {
		#[async_trait::async_trait]
		impl crate::api::utils::QbRequest for $req {
			type QueryObject = crate::api::utils::tags::TagExt;

			async fn build_query<G: crate::global::ApiGlobal>(
				&self,
				_: &std::sync::Arc<G>,
				access_token: &video_common::database::AccessToken,
			) -> tonic::Result<sqlx::QueryBuilder<'_, sqlx::Postgres>> {
				if self.tags.is_empty() {
					return Err(tonic::Status::invalid_argument("tags must be provided to remove a tag"));
				}

				crate::api::utils::tags::validate_tags_array(&self.tags)?;

				Ok(crate::api::utils::tags::remove_tag_query::<Self::Table>(&self.tags, pb::ext::UlidExt::into_ulid(self.id), Some(access_token.organization_id.0)))
			}
		}

		impl crate::api::utils::QbResponse for $resp {
			type Request = $req;

			fn from_query_object(query_object: Vec<<Self::Request as crate::api::utils::QbRequest>::QueryObject>) -> tonic::Result<Self> {
				if query_object.len() != 1 {
					return Err(tonic::Status::internal(format!(
						"failed to create {}, {} rows returned",
						<<Self::Request as crate::api::utils::TonicRequest>::Table as video_common::database::DatabaseTable>::FRIENDLY_NAME,
						query_object.len(),
					)));
				}

				Ok(Self {
					tags: Some(query_object.into_iter().next().unwrap().into_tags()?),
				})
			}
		}
	};
}

pub(crate) use impl_untag_req;