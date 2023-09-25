use argon2::{
    password_hash::{rand_core::OsRng, SaltString},
    Argon2, PasswordHash, PasswordHasher, PasswordVerifier,
};
use chrono::{DateTime, Utc};
use rand::Rng;
use uuid::Uuid;

use super::channel;

#[derive(Debug, Clone, Default, sqlx::FromRow)]
pub struct SearchResult {
    /// The user.
    #[sqlx(flatten)]
    pub user: Model,
    /// The similarity of the search query to the user's username.
    pub similarity: f64,
}

#[derive(Debug, Clone, Default, sqlx::FromRow)]
pub struct Model {
    /// The unique identifier for the user.
    pub id: Uuid,
    /// The username of the user.
    pub username: String,
    /// The display name of the user.
    pub display_name: String,
    /// The display color of the user.
    pub display_color: i32,
    /// The hashed password of the user. (argon2)
    pub password_hash: String,
    /// The email of the user.
    pub email: String,
    /// Whether the user has verified their email.
    pub email_verified: bool,
    /// The time the user last logged in.
    pub last_login_at: DateTime<Utc>,
    /// The time the user was last updated.
    pub updated_at: DateTime<Utc>,
    /// The time the user was created.
    pub profile_picture_id: Option<Uuid>,
    /// The roles of the user.
    pub roles: Vec<Uuid>,

    /// Channel
    #[sqlx(flatten)]
    pub channel: channel::Model,
}

impl Model {
    /// Uses argon2 to verify the password hash against the provided password.
    pub fn verify_password(&self, password: &str) -> bool {
        let hash = match PasswordHash::new(&self.password_hash) {
            Ok(hash) => hash,
            Err(err) => {
                tracing::error!("failed to parse password hash: {}", err);
                return false;
            }
        };

        Argon2::default()
            .verify_password(password.as_bytes(), &hash)
            .is_ok()
    }
}

/// Generates a new password hash using argon2.
pub fn hash_password(password: &str) -> String {
    let salt = SaltString::generate(&mut OsRng);

    let hash = Argon2::default()
        .hash_password(password.as_bytes(), &salt)
        .expect("failed to hash password");

    hash.to_string()
}

/// Validates a username.
pub fn validate_username(username: &str) -> Result<(), &'static str> {
    if username.len() < 3 {
        return Err("Username must be at least 3 characters long");
    }

    if username.len() > 20 {
        return Err("Username must be at most 20 characters long");
    }

    if !username
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '_')
    {
        return Err("Username must only contain alphanumeric characters and underscores");
    }

    Ok(())
}

/// Validates a password.
pub fn validate_password(password: &str) -> Result<(), &'static str> {
    if password.len() < 8 {
        return Err("Password must be at least 8 characters long");
    }

    if !password.chars().any(|c| c.is_ascii_lowercase()) {
        return Err("Password must contain at least one lowercase character");
    }

    if !password.chars().any(|c| c.is_ascii_uppercase()) {
        return Err("Password must contain at least one uppercase character");
    }

    if !password.chars().any(|c| c.is_ascii_digit()) {
        return Err("Password must contain at least one digit");
    }

    if !password.chars().any(|c| !c.is_ascii_alphanumeric()) {
        return Err("Password must contain at least one special character");
    }

    if password.len() > 100 {
        return Err("Password must be at most 100 characters long");
    }

    Ok(())
}

/// Validates an email.
pub fn validate_email(email: &str) -> Result<(), &'static str> {
    if email.len() < 5 {
        return Err("Email must be at least 5 characters long");
    }

    if email.len() > 100 {
        return Err("Email must be at most 100 characters long");
    }

    if !email.contains('@') {
        return Err("Email must contain an @");
    }

    if !email.contains('.') {
        return Err("Email must contain a .");
    }

    if !email_address::EmailAddress::is_valid(email) {
        return Err("Email is not a valid email address");
    }

    Ok(())
}

/// https://www.rapidtables.com/convert/color/hsl-to-rgb.html
fn hsl_to_rgb(h: u16, s: f64, l: f64) -> (u8, u8, u8) {
    let c = (1.0 - (2.0 * l - 1.0).abs()) * s;
    let x = c * (1.0 - ((h as f64 / 60.0) % 2.0 - 1.0).abs());
    let m = l - c / 2.0;
    let (r, g, b) = match h {
        0..=59 => (c, x, 0.0),
        60..=119 => (x, c, 0.0),
        120..=179 => (0.0, c, x),
        180..=239 => (0.0, x, c),
        240..=299 => (x, 0.0, c),
        300..=359 => (c, 0.0, x),
        _ => (0.0, 0.0, 0.0),
    };

    (
        ((r + m) * 255.0) as u8,
        ((g + m) * 255.0) as u8,
        ((b + m) * 255.0) as u8,
    )
}

pub fn generate_display_color() -> i32 {
    let (r, g, b) = hsl_to_rgb(rand::thread_rng().gen_range(0..=359), 1.0, 0.67);
    ((r as i32) << 16) + ((g as i32) << 8) + b as i32
}