use async_trait::async_trait;
use libunftp::auth::{Authenticator, Principal, Credentials, AuthenticationError};

#[derive(Debug)]
pub struct StaticAuthenticator {
    pub username: String,
    pub password: String,
}

#[async_trait]
impl Authenticator for StaticAuthenticator {
    async fn authenticate(
        &self,
        username: &str,
        credentials: &Credentials
    ) -> Result<Principal, AuthenticationError> {
        if let Some(password) = &credentials.password {
            if username == self.username && *password == self.password {
                println!("Received valid login from {} for {}", credentials.source_ip, username);
                return Ok(Principal { username: username.to_string() });
            }
        }
        println!("Received invalid login from {} for {}", credentials.source_ip, username);
        Err(AuthenticationError::BadPassword)
    }
}
