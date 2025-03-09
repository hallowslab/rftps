use async_trait::async_trait;
use libunftp::auth::{Authenticator, DefaultUser, Credentials, AuthenticationError};

#[derive(Debug)]
pub struct StaticAuthenticator {
    pub username: String,
    pub password: String,
}

#[async_trait]
impl Authenticator<DefaultUser> for StaticAuthenticator {
    async fn authenticate(
        &self,
        username: &str,
        credentials: &Credentials
    ) -> Result<DefaultUser, AuthenticationError> {
        if let Some(password) = &credentials.password {  // Borrow credentials.password here because move occurs because `password` has type `std::string::String`, which does not implement the `Copy` trait
            if username == self.username && *password == self.password { // dereference password here because the trait `PartialEq<std::string::String>` is not implemented for `&std::string::String`
                println!("Received valid login from {} for {}", credentials.source_ip, username);
                return Ok(DefaultUser);
            }
        }
        println!("Received invalid login from {} for {}", credentials.source_ip, username);
        Err(AuthenticationError::BadPassword)
    }
}
