use crate::permissions::SqlHandler;
use serenity::{
	client::Context,
	framework::{
		standard::{
			macros::{command, group},
			CommandResult,
		},
		StandardFramework,
	},
	model::channel::Message,
};
use std::sync::Arc;

#[group]
#[commands(listservers)]
struct ServerCommands;

pub struct ServerManager {
	sql_handler: Arc<SqlHandler>,
}

impl ServerManager {
	pub fn new(framework: &mut StandardFramework, sql_handler: Arc<SqlHandler>) -> Arc<Self> {
		framework.group_add(&SERVERCOMMANDS_GROUP);
		Arc::new(Self { sql_handler })
	}
}

#[command]
pub async fn listservers(ctx: &Context, msg: &Message) -> CommandResult {
	Ok(())
}
