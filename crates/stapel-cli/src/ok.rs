//! `stapel ok [KEY] <section>`: a person confirms a section (STP-2 AC-5, AC-16).

use crate::repo;
use stapel_core::confirm::prepare;
use stapel_core::dialog::ok_facts;
use stapel_core::grant::consume;
use stapel_core::identity::{in_agent_shell, user_name};
use stapel_core::state::{Confirmation, save};
use stapel_core::time::now_rfc3339;

pub fn run(key: Option<&str>, section: &str, grant: Option<&str>) -> Result<(), String> {
    if in_agent_shell() && grant.is_none() {
        return Err(format!(
            "only a person confirms a section: this shell belongs to an agent (CLAUDECODE or \
             CLAUDE_CODE_ENTRYPOINT is set); ask the agent to run `stapel ok` so that Claude Code \
             shows you the permission dialog, or, from a Zed task started from a Claude Code shell, \
             run `env -u CLAUDECODE -u CLAUDE_CODE_ENTRYPOINT stapel ok {section}`"
        ));
    }
    let (root, config) = repo::open()?;
    let by = user_name(&root)?;
    if by.contains(char::is_whitespace) {
        eprintln!(
            "warning: the name \"{by}\" from git config user.name goes into the tracked state.json"
        );
    }
    let mut prepared = prepare(&root, &config, key, section)?;
    if let Some(token) = grant {
        consume(&root, token, &ok_facts(&prepared))?;
    }
    for older in prepared
        .state
        .confirmations
        .iter_mut()
        .filter(|c| c.section == prepared.section)
    {
        older.text = None;
    }
    prepared.state.confirmations.push(Confirmation {
        section: prepared.section.clone(),
        by,
        at: now_rfc3339(),
        hash: prepared.hash.clone(),
        normal_form: stapel_core::hash::NORMAL_FORM,
        depends_on: prepared.depends_on,
        text: Some(prepared.text),
        via: Some(if grant.is_some() { "grant" } else { "terminal" }.into()),
        extra: Default::default(),
    });
    save(&prepared.dir.join("state.json"), &prepared.state)?;
    let short = &prepared.hash[..prepared.hash.len().min("sha256:".len() + 12)];
    println!("confirmed: {} {} ({short})", prepared.key, prepared.section);
    Ok(())
}
