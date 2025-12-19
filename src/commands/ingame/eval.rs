use std::collections::BTreeMap;

use fasteval::{Evaler, Parser, Slab};
use poise::serenity_prelude::Context;

use crate::{EvalUser, commands::prelude::Error};

async fn send_msg(tx: &tokio::sync::mpsc::Sender<String>, message: &str) -> Result<(), Error> {
    if message.trim().is_empty() {
        return Ok(());
    }
    let cmd = format!("MSG {}", message);
    tx.send(cmd).await.expect("Taurus dead");
    Ok(())
}

async fn eval_internal(ctx: &Context, username: &str, eval_string: &str) -> Result<String, Error> {
    let parser = Parser::new();
    let mut slab = Slab::new();
    let mut data = ctx.data.write().await;
    let namespaces = data
        .get_mut::<crate::EvalRepl>()
        .expect("TaurusChannel not found");
    let ns = namespaces
        .entry(username.to_owned())
        .or_insert_with(|| EvalUser {
            ns: Vec::from([BTreeMap::new()]),
        });
    let mut ans_key = "_".to_string();

    let mut line = eval_string.trim().to_string();
    if line.is_empty() {
        return Ok("".to_string());
    }

    let pieces: Vec<&str> = line.split_whitespace().collect();
    if pieces[0] == "let" {
        if pieces.len() < 4 || pieces[2] != "=" {
            return Ok("incorrect 'let' syntax. Should be: let x = ...".to_string());
        }
        ans_key = pieces[1].to_string();
        line = pieces[3..].join(" ");
    } else if pieces[0] == "clear" {
        ns.ns.clear();
        return Ok("Cleared all variables".to_string());
    } else if pieces[0] == "push" {
        ns.ns.push(BTreeMap::new());
        return Ok(format!("Entered scope[{}]", ns.ns.len() - 1));
    } else if pieces[0] == "pop" {
        ns.ns.pop();
        if ns.ns.is_empty() {
            ns.ns.push(BTreeMap::new());
        } // All scopes have been removed.  Add a new one.

        return Ok(format!("Exited scope[{}]", ns.ns.len()));
    }
    let expr_ref = match parser.parse(&line, &mut slab.ps) {
        Ok(expr_i) => slab.ps.get_expr(expr_i),
        Err(err) => {
            return Ok("parse error".to_string());
        }
    };

    let ans = match expr_ref.eval(&slab, &mut ns.ns) {
        Ok(val) => val,
        Err(err) => {
            return Ok("evaluation error".to_string());
        }
    };
    if ans_key != "_" {
        ns.ns.last_mut().unwrap().insert("_".to_string(), ans);
    }
    ns.ns.last_mut().unwrap().insert(ans_key, ans);
    Ok(format!("{}", ans))
}

pub async fn eval(ctx: &Context, username: &str, eval_string: &str) -> Result<(), Error> {
    let result = eval_internal(ctx, username, eval_string).await?;
    let message = format!("{}", result);
    let data = ctx.data.read().await;
    let (tx, _rx) = data
        .get::<crate::TaurusChannel>()
        .expect("TaurusChannel not found");
    send_msg(tx, &message).await?;
    Ok(())
}
