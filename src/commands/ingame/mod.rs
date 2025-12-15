mod score;


use poise::serenity_prelude::Context;

use crate::commands::prelude::Error;

pub async fn execute_ingame_command(ctx: &Context, server: &str, command: &str, args: &[&str]) -> Result<(), Error> {
    match command {
        "score" => {
            let Some(board) = args.get(0) else {
                return Ok(())
            };
            score::score(ctx, server, board);
        }
        _ => {}
    }
    Ok(())
}
