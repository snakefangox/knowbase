use actix_web::cookie::Key;
use comrak::{
    nodes::{AstNode, NodeValue},
    Arena, ComrakExtensionOptions, ComrakOptions, ComrakParseOptions, ComrakRenderOptions,
};
use lazy_static::lazy_static;
use redis::AsyncCommands;
use regex::Regex;
use serde::{Deserialize, Serialize};

lazy_static! {
    static ref INDEX_RE: Regex = Regex::new(r"(?s)\+\+\+INDEX\+\+\+\n(.*?)\n---INDEX---").unwrap();
}

#[derive(Debug, Clone)]
pub struct State {
    name: String,
    client: redis::Client,
    access_code: String,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct Page {
    pub content: String,
    pub index: String,
}

impl State {
    pub fn new() -> Self {
        let name = std::env::var("OVERMIND_NAME").unwrap_or("Overmind".to_owned());
        let access_code =
            std::env::var("OVERMIND_ACCESS_CODE").expect("OVERMIND_ACCESS_CODE should be set");
        let client = redis::Client::open(
            std::env::var("OVERMIND_REDIS_URL").expect("OVERMIND_REDIS_URL should be set"),
        )
        .expect("Redis URL should be valid");

        Self {
            client,
            name,
            access_code,
        }
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn is_access_code_correct(&self, password: &str) -> bool {
        self.access_code == password.trim()
    }

    pub async fn con(&self) -> redis::aio::Connection {
        self.client
            .get_async_connection()
            .await
            .expect("Redis should be available")
    }

    pub async fn master_key(&self) -> Key {
        let master_key: Option<Vec<u8>> = self.con().await.get("master_key").await.unwrap();
        let master_bytes = if master_key.is_none() {
            let key = Key::generate().master().to_vec();
            self.con()
                .await
                .set::<&str, Vec<u8>, ()>("master_key", key.clone())
                .await
                .unwrap();
            key
        } else {
            master_key.unwrap()
        };

        Key::from(&master_bytes)
    }

    pub async fn get_page(&self, path: &str) -> Option<Page> {
        let page_json: Option<String> = self
            .con()
            .await
            .get(format!("page/{}", path))
            .await
            .unwrap();

        page_json.map(|p| serde_json::from_str(&p).unwrap())
    }

    pub async fn set_page(&self, path: &str, mut md: String) {
        let arena = Arena::new();
        let opts = ComrakOptions {
            extension: ComrakExtensionOptions {
                strikethrough: true,
                tagfilter: true,
                table: true,
                autolink: true,
                tasklist: true,
                superscript: true,
                ..Default::default()
            },
            parse: ComrakParseOptions::default(),
            render: ComrakRenderOptions {
                hardbreaks: true,
                ..Default::default()
            },
        };

        let mut page = Page::default();

        if let Some(index_match) = INDEX_RE.captures(&md) {
            page.index.push_str(&comrak::markdown_to_html(
                index_match.get(1).unwrap().as_str(),
                &opts,
            ));
            md.replace_range(index_match.get(0).unwrap().range(), "");
        }

        let root = comrak::parse_document(&arena, &md, &opts);
        iter_md_nodes(root, &|n| match &mut n.data.borrow_mut().value {
            &mut NodeValue::Link(ref mut link) => {
                if link.url.starts_with("/") {
                    link.url.insert_str(0, "/w");
                }
            }
            _ => (),
        });

        let mut html = Vec::new();
        comrak::format_html(root, &opts, &mut html).unwrap();
        page.content.push_str(&String::from_utf8(html).unwrap());

        self.con()
            .await
            .set(
                format!("page/{}", path),
                &serde_json::to_string(&page).unwrap(),
            )
            .await
            .unwrap()
    }
}

fn iter_md_nodes<'a, F>(node: &'a AstNode<'a>, f: &F)
where
    F: Fn(&'a AstNode<'a>),
{
    f(node);
    for c in node.children() {
        iter_md_nodes(c, f);
    }
}
