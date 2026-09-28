use clap::Arg;
use clap::ArgAction;
use clap::Command;
use clap::error::ErrorKind;
use clap::value_parser;

fn parse_define(raw: &str) -> Result<(String, String), String> {
    let Some((k, v)) = raw.split_once('=') else {
        return Err(format!("invalid define `{raw}`: expected key=value"));
    };
    if v.contains('=') || k.is_empty() || v.is_empty()
        || k.chars().any(char::is_whitespace) || v.chars().any(char::is_whitespace)
    {
        return Err(format!("invalid define `{raw}`"));
    }
    Ok((k.to_owned(), v.to_owned()))
}
fn parse_port(raw: &str) -> Result<u16, String> {
    if raw.is_empty() || !raw.bytes().all(|b| b.is_ascii_digit()) {
        return Err(format!("invalid port `{raw}`"));
    }
    match raw.parse::<u32>() {
        Ok(p) if (1..=65535).contains(&p) => Ok(p as u16),
        _ => Err(format!("invalid port `{raw}`")),
    }
}

fn tool() -> Command {
    Command::new("tool")
        .after_help("External plugins: any other <COMMAND> is captured as a root plugin.")
        .arg(Arg::new("config").long("config").global(true).default_value("builtin.toml").value_name("path"))
        .arg(Arg::new("define").long("define").value_name("key=value").action(ArgAction::Append).value_parser(parse_define))
        .subcommand_required(true)
        .arg_required_else_help(true)
        .allow_external_subcommands(true)
        .external_subcommand_value_parser(value_parser!(String))
        .subcommand(
            Command::new("serve")
                .alias("srv").visible_alias("serve-http")
                .after_help("External plugins: any other <COMMAND> is captured as a serve plugin.")
                .subcommand_required(true).arg_required_else_help(true)
                .allow_external_subcommands(true)
                .external_subcommand_value_parser(value_parser!(String))
                .subcommand(
                    Command::new("worker").alias("wk").visible_alias("worker-http")
                        .arg(Arg::new("port").long("port").value_name("port").required(true).value_parser(parse_port))
                        .arg(Arg::new("http").long("http").action(ArgAction::SetTrue).conflicts_with("https"))
                        .arg(Arg::new("https").long("https").action(ArgAction::SetTrue).conflicts_with("http"))
                        .arg(Arg::new("trailing").value_name("TRAILING").action(ArgAction::Append).num_args(1..).last(true)),
                ),
        )
        .subcommand(
            Command::new("check").alias("chk").visible_alias("check-all")
                .arg(Arg::new("path").value_name("PATH").action(ArgAction::Set))
                .arg(Arg::new("strict").long("strict").action(ArgAction::SetTrue)),
        )
}

fn run(argv: &[&str]) {
    match tool().try_get_matches_from(argv) {
        Ok(m) => {
            let (n1, s1) = m.subcommand().unwrap();
            let ext = s1.get_many::<String>("").is_some();
            let info = if ext {
                let toks: Vec<_> = std::iter::once(n1.to_owned()).chain(s1.get_many::<String>("").unwrap().cloned()).collect();
                format!("ROOT-EXT {toks:?} define={:?}", m.get_many::<(String,String)>("define").map(|v| v.cloned().collect::<Vec<_>>()).unwrap_or_default())
            } else if n1 == "serve" {
                if let Some((n2, s2)) = s1.subcommand() {
                    if s2.get_many::<String>("").is_some() {
                        format!("SERVE-EXT {:?}", std::iter::once(n2.to_owned()).chain(s2.get_many::<String>("").unwrap().cloned()).collect::<Vec<_>>())
                    } else {
                        format!("serve>{n2} port={:?} http={} https={} trailing={:?}",
                            s2.get_one::<u16>("port"), s2.get_flag("http"), s2.get_flag("https"),
                            s2.get_many::<String>("trailing").map(|v| v.cloned().collect::<Vec<_>>()).unwrap_or_default())
                    }
                } else { "serve(no sub)".into() }
            } else {
                format!("{n1} path={:?} strict={}", s1.get_one::<String>("path"), s1.get_flag("strict"))
            };
            println!("OK   {argv:?}\n     {info}");
        }
        Err(e) => {
            let txt = e.to_string();
            let head = txt.lines().take(3).collect::<Vec<_>>().join(" / ");
            println!("ERR  {argv:?}: {:?}\n     {head}", e.kind());
        }
    }
}

fn main() {
    let cases: &[&[&str]] = &[
        &["tool", "--define", "a=1", "srv", "--config", "c.toml", "wk", "--port", "8080"],
        &["tool", "srv", "wk", "--port", "80", "--http", "--", "srv", "wk", "--"],
        &["tool", "srv", "plug", "--port", "0"],
        &["tool", "sr", "x"],
        &["tool", "srv", "work", "x"],
        &["tool", "w"],
        &["tool", "srv", "wk"],
        &["tool", "srv", "wk", "--port", "80", "--port", "90"],
        &["tool", "srv", "wk", "--port", "80", "--http", "--https"],
        &["tool", "chk", "p", "--strict"],
        &["tool", "--", "srv", "x"],
        &["tool", "srv", "--", "wk"],
        &["tool", "srv", "--define", "a=1"],
        &["tool", "srv", "wk", "--port"],
        &["tool", "srv", "wk", "--help"],
        &["tool", "serve-http"],
    ];
    for c in cases { run(c); }
    let _ = ErrorKind::InvalidValue;
}
