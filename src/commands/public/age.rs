use chrono::{DateTime, Datelike, Timelike};
use poise::{command, CreateReply};
use timeago::Formatter;

use crate::commands::{embed, Context, Error};

/// Displays the age of the Hypnos Server
#[command(slash_command, prefix_command)]
pub async fn age(
    ctx: Context<'_>,
) -> Result<(), Error> {
    let mut f = Formatter::new();
    let start = DateTime::from_timestamp_millis(1569559890000)
    .expect("Failed to parse start date");
    let now = chrono::Utc::now();
    f.max_unit(timeago::TimeUnit::Years);
    f.num_items(99);
    f.ago("");
    let age = f.convert_chrono(start, now);
    let embed = embed(&ctx).await?
        .title("Age of the Hypnos Server")
        .description(&age);
    let reply = CreateReply::default()
        .embed(embed);
    ctx.send(reply).await?;

    Ok(())
}