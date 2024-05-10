use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

use serde::Serialize;

use crate::imp::database::{mysql::MysqlConn, DbConnectionDetails, TableConfig};
use crate::User;

use super::{
    auth::{create_jwt, JwtSession},
    config::{Config, Database, SourceType},
    AppError, SharedConnection,
};

pub(crate) type TableList = Vec<BasableTable>;

#[derive(Serialize, Clone)]
pub(crate) struct BasableTable {
    pub name: String,
    pub row_count: u32,
    pub col_count: u32,
    pub created: Option<String>,
    pub updated: Option<String>,
}

/// Basable base trait that must be implemented by every instance of connection in Basable.
///
/// Check `imp` module for different implementations of this trait.
pub(crate) trait BasableConnection: Send + Sync {
    type Error;
    /// A new instance of BasableConnection
    fn new(conn: Config) -> Result<Self, Self::Error>
    where
        Self: Sized;

    /// Details about the connection
    fn details(&self) -> Result<DbConnectionDetails, Self::Error>;

    /// Load table summaries
    fn load_tables(&self) -> Result<TableList, Self::Error>;

    /// Check if a table with the given name exists in the database connection.
    fn table_exists(&self, name: &str) -> Result<bool, Self::Error>;

    /// Saves a table configuration. If `save_local` is true, it saves in memore using 
    /// `BasableConnection` instance. Otherwise, it saves to remote server.
    fn save_table_config(
        &mut self,
        table_name: &str,
        table_config: TableConfig,
        save_local: bool,
    ) -> Result<(), Self::Error>;
}

#[derive(Default)]
pub(crate) struct Basable {
    pub users: HashMap<String, User>,
    pub connections: HashMap<String, SharedConnection>,
}

impl Basable {
    /// Creates a new thread-safe instance of `BasableConnection` as required by the `Config` parameter.
    pub(crate) fn create_connection(config: &Config) -> Result<Option<SharedConnection>, AppError> {
        let conn = match config.source_type() {
            SourceType::Database(db) => match db {
                Database::Mysql => MysqlConn::new(config.clone())?,
                _ => todo!(),
            },
            _ => todo!(),
        };

        Ok(Some(Arc::new(Mutex::new(conn))))
    }

    /// Gets a user's active `BasableConnection`.
    pub(crate) fn get_connection(&self, user_id: &str) -> Option<&SharedConnection> {
        self.connections.get(user_id)
    }

    /// Creates a new guest user using the request `SocketAddr`
    pub(crate) fn create_guest_user(&mut self, req_ip: &str) -> Result<JwtSession, AppError> {
        let session_id = create_jwt(req_ip)?; // jwt encode the ip

        let user = User {
            id: req_ip.to_owned(),
            is_logged: false,
        };

        self.add_user(user.clone());

        Ok(session_id)
    }

    /// Saves the `Config` to Basable's remote server in association with the user_id
    pub(crate) fn save_config(&mut self, config: &Config, user_id: &str) {
        let user = self
            .find_user(user_id)
            .expect("Unable to find user with id");
        user.save_config(config);
    }

    /// Get an active `User` with the `user_id` from Basable's active users.
    pub(crate) fn find_user(&self, user_id: &str) -> Option<&User> {
        self.users.get(user_id)
    }

    /// Remove the user from Basable's active users.
    pub(crate) fn log_user_out(&mut self, user_id: &str) {
        if let Some(user) = self.find_user(user_id) {
            user.logout();
            self.users.remove(user_id);
        }
    }

    /// Adds a user to Basable's active user.
    fn add_user(&mut self, user: User) {
        let id = user.id.clone();
        self.users.insert(id, user);
    }

    /// Adds a `BasableConnection` to active connections.
    pub(crate) fn add_connection(&mut self, user_id: String, conn: SharedConnection) {
        // TODO: Find and close existing connection before insert a new one.
        self.connections.insert(user_id, conn);
    }
}
