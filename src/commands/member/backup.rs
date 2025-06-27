use poise::{CreateReply, serenity_prelude::CreateEmbed};

use crate::{TaurusChannel, commands::prelude::*, fetch_latest_with_type};

use super::check_role;

async fn send_and_recieve(ctx: Context<'_>, cmd: String, args: String) -> Result<String, Error> {
    let cache = {
        let data = ctx.serenity_context().data.read().await;
        let (sender, cache) = data
            .get::<TaurusChannel>()
            .expect("TaurusChannel not found in context data");
        sender.send(format!("{} {}", cmd, args)).await?;
        cache.clone()
    };
    let response = fetch_latest_with_type(cache, &cmd).await?;
    Ok(response)
}

/// Gets the invite link for the Hypnos Discord server
///
/// # Arguments
/// * `cmd` - The command to execute, which can be one of the following:
///     - `ls`: List the contents of the backup directory
///     - `rm`: Remove a file from the backup directory
///     - `new`: Create a new backup
#[command(
    slash_command,
    prefix_command,
    subcommands("ls", "rm", "new"),
    subcommand_required,
    check = "check_role"
)]
pub async fn backup(_: Context<'_>) -> Result<(), Error> {
    Ok(())
}

async fn gather_response(
    ctx: Context<'_>,
    cmd: String,
    args: String,
    title: &str,
) -> Result<CreateReply, Error> {
    let response = send_and_recieve(ctx.clone(), cmd, args).await?;

    let embed = CreateEmbed::default().title(title).description(response);

    Ok(CreateReply::default().embed(embed))
}

/// Lists the backups available
#[command(slash_command, prefix_command, aliases("ls", "list"))]
pub async fn ls(ctx: Context<'_>) -> Result<(), Error> {
    let reply = gather_response(
        ctx.clone(),
        "LIST_BACKUPS".to_string(),
        String::new(),
        "Backup List",
    )
    .await?;
    ctx.send(reply).await?;
    Ok(())
}

/// Removes a backup by name
///
/// # Arguments
/// * `backup_name` - The name of the backup to remove
#[command(slash_command, prefix_command, aliases("rm", "remove", "delete"))]
async fn rm(
    ctx: Context<'_>,
    #[description = "Name of the backup to remove"] backup_name: String,
) -> Result<(), Error> {
    let reply = gather_response(
        ctx.clone(),
        "RM_BACKUP".to_string(),
        backup_name,
        "Remove Backup",
    )
    .await?;
    ctx.send(reply).await?;
    Ok(())
}

/// Creates a new backup with the specified name
///
/// # Arguments
/// * `backup_name` - The name of the backup to create
#[command(slash_command, prefix_command, aliases("new", "create"))]
async fn new(
    ctx: Context<'_>,
    #[description = "Name of the session to create a backup for"] backup_name: String,
) -> Result<(), Error> {
    let reply = gather_response(
        ctx.clone(),
        "BACKUP".to_string(),
        backup_name,
        "Create Backup",
    )
    .await?;
    ctx.send(reply).await?;
    Ok(())
}
