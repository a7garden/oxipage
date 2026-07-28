use crate::output::Output;
use crate::sites;
use clap::Subcommand;
use serde_json::json;

#[derive(Subcommand, Debug, Clone)]
pub enum SiteCommand {
    /// 새 사이트 등록
    Add {
        name: String,
        #[arg(long)]
        endpoint: String,
        #[arg(long)]
        token: Option<String>,
        #[arg(long)]
        default: bool,
    },
    /// 사이트 목록
    List,
    /// 사이트 상세 정보
    Show {
        name: Option<String>,
    },
    /// 기본 사이트로 전환
    Use {
        name: String,
    },
    /// 사이트 정보 수정
    Edit {
        name: String,
        #[arg(long)]
        endpoint: Option<String>,
        #[arg(long)]
        token: Option<String>,
    },
    /// 사이트 삭제
    Rm {
        name: String,
    },
}

pub(crate) async fn dispatch_site(
    cmd: &SiteCommand,
    out: &Output,
    sites_file: &sites::SitesFile,
    active_site: Option<&str>,
) -> anyhow::Result<()> {
    match cmd {
        SiteCommand::List => site_list(out, sites_file, active_site),
        SiteCommand::Show { name } => site_show(out, sites_file, name.as_deref(), active_site),
        SiteCommand::Use { name } => site_use(out, sites_file, name).await,
        SiteCommand::Add { name, endpoint, token, default } => {
            site_add(out, sites_file, name, endpoint, token.as_deref(), *default).await
        }
        SiteCommand::Edit { name, endpoint, token } => {
            site_edit(out, sites_file, name, endpoint.as_deref(), token.as_deref()).await
        }
        SiteCommand::Rm { name } => site_rm(out, sites_file, name).await,
    }
}

fn site_list(out: &Output, sites_file: &sites::SitesFile, active_site: Option<&str>) -> anyhow::Result<()> {
    if out.json {
        let list: Vec<serde_json::Value> = sites_file
            .site_names()
            .into_iter()
            .map(|name| {
                let entry = sites_file.get(&name);
                json!({
                    "name": name,
                    "endpoint": entry.map(|e| e.endpoint.as_str()).unwrap_or(""),
                    "active": Some(name.as_str()) == active_site,
                    "has_token": entry.and_then(|e| e.token.as_ref()).is_some(),
                })
            })
            .collect();
        println!("{}", serde_json::to_string_pretty(&list)?);
    } else {
        let names = sites_file.site_names();
        if names.is_empty() {
            println!("no sites configured — use `oxipage site add`");
            return Ok(());
        }
        for name in &names {
            let entry = sites_file.get(name);
            let marker = if Some(name.as_str()) == active_site { "* " } else { "  " };
            let ep = entry.map(|e| e.endpoint.as_str()).unwrap_or("?");
            println!("{marker}{name}   {ep}");
        }
    }
    Ok(())
}

fn site_show(
    out: &Output,
    sites_file: &sites::SitesFile,
    name: Option<&str>,
    active_site: Option<&str>,
) -> anyhow::Result<()> {
    let target = name.or(active_site)
        .and_then(|n| sites_file.get(n).map(|e| (n, e)));
    let (name, entry) = match target {
        Some(pair) => pair,
        None => {
            anyhow::bail!("no active site and no site name given");
        }
    };
    if out.json {
        let masked = entry.token.as_ref().map(|t| {
            if t.len() > 8 {
                format!("{}...{}", &t[..6], &t[t.len()-3..])
            } else {
                "***".into()
            }
        });
        println!("{}", serde_json::to_string_pretty(&json!({
            "name": name,
            "endpoint": entry.endpoint,
            "token": masked,
            "active": Some(name) == active_site,
        }))?);
    } else {
        let active_label = if Some(name) == active_site { " (active)" } else { "" };
        println!("name:       {name}{active_label}");
        println!("endpoint:   {}", entry.endpoint);
        if let Some(tok) = &entry.token {
            let masked = if tok.len() > 8 {
                format!("{}...{}", &tok[..6], &tok[tok.len()-3..])
            } else {
                "***".into()
            };
            println!("token:      {masked}");
        } else {
            println!("token:      (none)");
        }
    }
    Ok(())
}

async fn site_use(
    out: &Output,
    sites_file: &sites::SitesFile,
    name: &str,
) -> anyhow::Result<()> {
    if !sites_file.exists(name) {
        anyhow::bail!("site '{name}' not found");
    }
    let mut new_sites = sites_file.clone();
    new_sites.default_site = Some(name.to_string());
    new_sites.save()?;
    if !out.json {
        println!("active site set to '{name}'");
    }
    Ok(())
}

async fn site_add(
    out: &Output,
    sites_file: &sites::SitesFile,
    name: &str,
    endpoint: &str,
    token: Option<&str>,
    default: bool,
) -> anyhow::Result<()> {
    if sites_file.exists(name) {
        anyhow::bail!("site '{name}' already exists — use `oxipage site edit` to update");
    }
    let mut new_sites = sites_file.clone();
    new_sites.sites.insert(
        name.to_string(),
        sites::SiteEntry {
            endpoint: endpoint.to_string(),
            token: token.map(String::from),
        },
    );
    if default || new_sites.default_site.is_none() {
        new_sites.default_site = Some(name.to_string());
    }
    new_sites.save()?;
    if !out.json {
        if default {
            println!("site '{name}' added and set as default");
        } else {
            println!("site '{name}' added");
        }
    }
    Ok(())
}

async fn site_edit(
    out: &Output,
    sites_file: &sites::SitesFile,
    name: &str,
    endpoint: Option<&str>,
    token: Option<&str>,
) -> anyhow::Result<()> {
    let _entry = sites_file.sites.get(name).ok_or_else(|| {
        anyhow::anyhow!("site '{name}' not found — use `oxipage site add` first")
    })?;
    let mut new_sites = sites_file.clone();
    let new_entry = new_sites.sites.get_mut(name).unwrap();
    if let Some(ep) = endpoint {
        new_entry.endpoint = ep.to_string();
    }
    // token=Some("") → clear token; token=Some("tok") → set; token=None → leave as-is
    match token {
        Some("") => new_entry.token = None,
        Some(t) => new_entry.token = Some(t.to_string()),
        None => {},
    }
    new_sites.save()?;
    if !out.json {
        println!("site '{name}' updated");
    }
    Ok(())
}

async fn site_rm(
    out: &Output,
    sites_file: &sites::SitesFile,
    name: &str,
) -> anyhow::Result<()> {
    if !sites_file.exists(name) {
        anyhow::bail!("site '{name}' not found");
    }
    let mut new_sites = sites_file.clone();
    new_sites.sites.remove(name);
    // If removed site was the default, pick the next available or clear.
    if new_sites.default_site.as_deref() == Some(name) {
        let next = new_sites.site_names().into_iter().next();
        new_sites.default_site = next;
    }
    new_sites.save()?;
    if !out.json {
        println!("site '{name}' removed");
    }
    Ok(())
}

// ───────────────────────── init / status / serve ─────────────────────────
