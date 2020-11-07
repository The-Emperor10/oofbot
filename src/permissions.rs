use num_derive::*;
use serenity::framework::standard::CommandResult;
use serenity::framework::StandardFramework;
use serenity::prelude::Mutex;
use serenity::{
	model::id::{ChannelId, GuildId, RoleId, UserId},
	prelude::TypeMapKey,
};
use sqlite::{Connection, Error as SQLiteError, Value};
use std::sync::Arc;

impl TypeMapKey for SqlHandler {
	type Value = Arc<SqlHandler>;
}
pub struct SqlHandler {
	pub sql_connection: Mutex<Connection>,
}
#[repr(u64)]
#[derive(FromPrimitive, ToPrimitive, Clone, Debug)]
pub enum Permission {
	SomeonePing = 0,
	ManageCanaryUpdate = 1,
	ManageDogebotInsults = 2,
	ManagePermissions = 3,
	ManageServers = 4,
	Server = 5,
}

pub fn do_framework(_framework: &mut StandardFramework) {}

impl SqlHandler {
	pub fn new() -> Arc<Self> {
		let sql_connection = Mutex::new(Connection::open("oofbot.db").unwrap());
		Arc::new(Self { sql_connection })
	}
	/// Creates the sqlite canary update table
	pub async fn create_canary_table(&self) -> CommandResult {
		self.sql_connection.lock().await.execute("CREATE TABLE canary (guild_id UNSIGNED BIG INT UNIQUE NOT NULL, channel_id UNSIGNED BIG INT UNIQUE NOT NULL)")?;
		Ok(())
	}
	pub async fn create_dogebot_table(&self) -> CommandResult {
		self.sql_connection.lock().await.execute("CREATE TABLE dogebot (guild_id UNSIGNED BIG INT UNIQUE NOT NULL, channel_id UNSIGNED BIG INT UNIQUE NOT NULL)")?;
		Ok(())
	}
	pub async fn create_permission_role_table(&self) -> CommandResult {
		self.sql_connection.lock().await.execute("CREATE TABLE permission_role (guild_id UNSIGNED BIG INT NOT NULL, role_id UNSIGNED BIG INT NOT NULL channel_id UNSIGNED BIG INT, permission_id UNSIGNED BIG INT NOT NULL, data BLOB)")?;
		Ok(())
	}
	pub async fn create_permission_user_table(&self) -> CommandResult {
		self.sql_connection.lock().await.execute("CREATE TABLE permission_user (guild_id UNSIGNED BIG INT NOT NULL, user_id UNSIGNED BIG INT NOT NULL channel_id UNSIGNED BIG INT, permission_id UNSIGNED BIG INT NOT NULL, data BLOB)")?;
		Ok(())
	}
	pub async fn create_server_table(&self) -> CommandResult {
		self.sql_connection
			.lock().await
			.execute("CREATE TABLE servers (user_id UNSIGNED BIG INT NOT NULL, server_name TEXT NOT NULL UNIQUE)")?;
		Ok(())
	}
	pub async fn register_permission_role(
		&self,
		guild_id: GuildId,
		role_id: RoleId,
		channel_id: Option<ChannelId>,
		permission: Permission,
	) -> CommandResult {
		let sql = self.sql_connection.lock().await;
		let mut cursor = sql
			.prepare("INSERT INTO permission_role VALUES (?, ?, ?, ?)")?
			.cursor();
		if let Some(channel_id) = channel_id {
			cursor.bind(&[
				Value::Integer(i64::from_ne_bytes(guild_id.0.to_ne_bytes())),
				Value::Integer(i64::from_ne_bytes(role_id.0.to_ne_bytes())),
				Value::Integer(i64::from_ne_bytes(channel_id.0.to_ne_bytes())),
				Value::Integer(i64::from_ne_bytes((permission as u64).to_ne_bytes())),
			])?;
		} else {
			cursor.bind(&[
				Value::Integer(i64::from_ne_bytes(guild_id.0.to_ne_bytes())),
				Value::Integer(i64::from_ne_bytes(role_id.0.to_ne_bytes())),
				Value::Null,
				Value::Integer(i64::from_ne_bytes((permission as u64).to_ne_bytes())),
			])?;
		}
		Ok(())
	}
	pub async fn check_permission_role(
		&self,
		guild_id: GuildId,
		role_id: RoleId,
		channel_id: Option<ChannelId>,
		permission: Permission,
	) -> Result<Option<Vec<u8>>, SQLiteError> {
		let sql = self.sql_connection.lock().await;
		let mut cursor = sql
			.prepare("SELECT role_id FROM permission_role WHERE guild_id = ?, role_id = ?, channel_id = ?, permission = ?")?
			.cursor();
		match channel_id {
			Some(channel_id) => cursor.bind(&[
				Value::Integer(i64::from_ne_bytes(guild_id.0.to_ne_bytes())),
				Value::Integer(i64::from_ne_bytes(role_id.0.to_ne_bytes())),
				Value::Integer(i64::from_ne_bytes(channel_id.0.to_ne_bytes())),
				Value::Integer(i64::from_ne_bytes((permission as u64).to_ne_bytes())),
			])?,
			None => cursor.bind(&[
				Value::Integer(i64::from_ne_bytes(guild_id.0.to_ne_bytes())),
				Value::Integer(i64::from_ne_bytes(role_id.0.to_ne_bytes())),
				Value::Null,
				Value::Integer(i64::from_ne_bytes((permission as u64).to_ne_bytes())),
			])?,
		};
		if let Some(row) = cursor.next()? {
			return Ok(Some(match row[row.len()].as_binary() {
				Some(x) => Vec::from(x),
				None => return Ok(None),
			}));
		}
		Ok(None)
	}

	pub async fn register_permission_user(
		&self,
		guild_id: GuildId,
		user_id: UserId,
		channel_id: Option<ChannelId>,
		permission: Permission,
	) -> CommandResult {
		let sql = self.sql_connection.lock().await;
		let mut cursor = sql
			.prepare("INSERT INTO permission_role VALUES (?, ?, ?, ?)")?
			.cursor();
		if let Some(channel_id) = channel_id {
			cursor.bind(&[
				Value::Integer(i64::from_ne_bytes(guild_id.0.to_ne_bytes())),
				Value::Integer(i64::from_ne_bytes(user_id.0.to_ne_bytes())),
				Value::Integer(i64::from_ne_bytes(channel_id.0.to_ne_bytes())),
				Value::Integer(i64::from_ne_bytes((permission as u64).to_ne_bytes())),
			])?;
		} else {
			cursor.bind(&[
				Value::Integer(i64::from_ne_bytes(guild_id.0.to_ne_bytes())),
				Value::Integer(i64::from_ne_bytes(user_id.0.to_ne_bytes())),
				Value::Null,
				Value::Integer(i64::from_ne_bytes((permission as u64).to_ne_bytes())),
			])?;
		}
		Ok(())
	}
	pub async fn check_permission_user(
		&self,
		guild_id: GuildId,
		user_id: UserId,
		channel_id: Option<ChannelId>,
		permission: Permission,
	) -> Result<bool, SQLiteError> {
		let sql = self.sql_connection.lock().await;
		let mut cursor = sql
			.prepare("SELECT role_id FROM permission_role WHERE guild_id = ?, role_id = ?, channel_id = ?, permission = ?")?
			.cursor();
		match channel_id {
			Some(channel_id) => cursor.bind(&[
				Value::Integer(i64::from_ne_bytes(guild_id.0.to_ne_bytes())),
				Value::Integer(i64::from_ne_bytes(user_id.0.to_ne_bytes())),
				Value::Integer(i64::from_ne_bytes(channel_id.0.to_ne_bytes())),
				Value::Integer(i64::from_ne_bytes((permission as u64).to_ne_bytes())),
			])?,
			None => cursor.bind(&[
				Value::Integer(i64::from_ne_bytes(guild_id.0.to_ne_bytes())),
				Value::Integer(i64::from_ne_bytes(user_id.0.to_ne_bytes())),
				Value::Null,
				Value::Integer(i64::from_ne_bytes((permission as u64).to_ne_bytes())),
			])?,
		};
		Ok(cursor.next()?.is_some())
	}
}
