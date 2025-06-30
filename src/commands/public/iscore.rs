use poise::serenity_prelude::CreateAttachment;
use resvg::{tiny_skia, usvg::{self, Tree}};

use super::score::get_scoreboard;

use crate::commands::{prelude::*, public::score::format_with_spaces};



const SVG1: &str = r#"<svg viewBox="0 0 "#;

const SVG2: &str = 
    r#"" xmlns="http://www.w3.org/2000/svg">
                <defs>
                    <style>
                        @font-face {
                            font-family: minecraft;
                            src: url('data/minecraft_font.ttf');
                        }
                    </style>
            </defs>
                <style>
                    text {
                        font-family: Minecraft, minecraft, sans-serif;
                    }
                    .title {
                        text-anchor: middle;
                        font-size: 60px;
                        fill: #FFFFFF;
                    }
                    .score {
                        text-anchor: end;
                        font-size: 60px;
                        fill: #FF5555;
                    }
                    .ign {
                        text-anchor: start;
                        font-size: 60px;
                        fill: #BFBFBF;
                    }
                    .total {
                        text-anchor: start;
                        font-size: 60px;
                        fill: #FFFFFF;
                    }
                </style>
                <rect height="100%" width="100%" fill='#36393F'></rect>
                <text class="title" x="50%" y="60">"#;

const SVG3: &str = r#"</text>"#;

const GLYPH_HEIGHT: u32 = 60;
const GLYPH_PADDING: u32 = 6;
const GLYPH_INTERVAL: u32 = GLYPH_HEIGHT + GLYPH_PADDING;

#[command(slash_command, prefix_command)]
pub async fn iscore(ctx: Context<'_>, board: String) -> Result<(), Error> {
    let scoreboard = get_scoreboard(ctx, &board).await;
    let scoreboard = match scoreboard {
        Some(scoreboard) => scoreboard,
        None => {
            ctx.send(
                CreateReply::default().content(format!("No scoreboard found for `{}`", board)),
            )
            .await?;
            return Ok(());
        }
    };
    let mut svg = format!("{} {} {} {} {} {}", SVG1, 900, (scoreboard.scores.len() as u32 + 1 + 1) * GLYPH_INTERVAL + GLYPH_PADDING * 2, SVG2, board, SVG3);
    svg += &format!(
        "<text class=\"total\" x=\"5px\" y=\"{}\">Total</text>
         <text class=\"score\" x=\"895px\" y=\"{}\">{}</text>",
        GLYPH_INTERVAL * 2,
        GLYPH_INTERVAL * 2,
        format_with_spaces(scoreboard.total)
    );
    let mut position = 1;
    for (score, value) in scoreboard.scores.iter() {
        let y = (position + 2) * GLYPH_INTERVAL;
        svg += &format!(
            "<text class=\"ign\" x=\"5px\" y=\"{}\">{}</text>
             <text class=\"score\" x=\"895px\" y=\"{}\">{}</text>",
            y,
            score,
            y,
            value
        );
        position += 1;
    }
    svg += "</svg>";

    let mut opt = usvg::Options::default();
    opt.fontdb_mut().load_font_file("data/minecraft_font.ttf")
        .map_err(|e| "Failed to load font: ".to_string() + &e.to_string())?;
    let tree = Tree::from_str(&svg, &opt)
        .map_err(|e| "Failed to parse SVG: ".to_string() + &e.to_string())?;

    let pixmap_size = tree.size().to_int_size();
    let mut pixmap = tiny_skia::Pixmap::new(pixmap_size.width(), pixmap_size.height())
        .ok_or("Failed to create Pixmap")?;
    resvg::render(&tree, tiny_skia::Transform::default(), &mut pixmap.as_mut());
    let bytes = pixmap.encode_png()
        .map_err(|e| "Failed to save scoreboard image: ".to_string() + &e.to_string())?;
    let attachment = CreateAttachment::bytes(bytes, "scoreboard.png".to_string());
    let reply = CreateReply::default().attachment(attachment);

    ctx.send(reply).await?;
    Ok(())
}
