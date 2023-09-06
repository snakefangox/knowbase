mod state;

use std::io::Read;

use actix_multipart::form::{tempfile::TempFile, MultipartForm};
use actix_session::{storage::CookieSessionStore, Session, SessionMiddleware};
use actix_web::{
    error::ErrorUnsupportedMediaType,
    get, post,
    web::{Data, Form},
    App, HttpRequest, HttpResponse, HttpServer, Responder, Result,
};
use askama_actix::Template;
use serde::Deserialize;
use state::{State, Page};

#[derive(Template)]
#[template(path = "index.html")]
struct IndexTemplate<'a> {
    name: &'a str,
    error: &'a str,
}

#[get("/")]
async fn index(req: HttpRequest, state: Data<State>, session: Session) -> Result<impl Responder> {
    let authed = session.get::<bool>("auth")?;
    if authed.is_some() && authed.unwrap() {
        Ok(HttpResponse::SeeOther()
            .append_header(("Location", "/w"))
            .body(()))
    } else {
        Ok(IndexTemplate {
            name: state.name(),
            error: "",
        }
        .respond_to(&req))
    }
}

#[derive(Deserialize)]
struct LoginForm {
    password: String,
}

#[post("/login")]
async fn login(
    req: HttpRequest,
    session: Session,
    state: Data<State>,
    form: Form<LoginForm>,
) -> Result<impl Responder> {
    if state.is_access_code_correct(&form.password) {
        session.insert("auth", true)?;
        Ok(HttpResponse::SeeOther()
            .append_header(("Location", "/w"))
            .body(()))
    } else {
        Ok(IndexTemplate {
            name: state.name(),
            error: "Invalid access code",
        }
        .respond_to(&req))
    }
}

#[derive(Template)]
#[template(path = "upload.html")]
struct UploadTemplate<'a> {
    name: &'a str,
    message: &'a str,
}

#[get("/upload")]
async fn upload_page(
    req: HttpRequest,
    session: Session,
    state: Data<State>,
) -> Result<impl Responder> {
    let authed = session.get::<bool>("auth")?;
    if authed.is_none() || !authed.unwrap() {
        return Ok(HttpResponse::SeeOther()
            .append_header(("Location", "/"))
            .body(()));
    }

    Ok(UploadTemplate {
        name: state.name(),
        message: "",
    }
    .respond_to(&req))
}

#[derive(MultipartForm)]
struct UploadForm {
    zip_file: TempFile,
}

#[post("/upload")]
async fn upload_file(
    req: HttpRequest,
    session: Session,
    state: Data<State>,
    payload: MultipartForm<UploadForm>,
) -> Result<impl Responder> {
    let authed = session.get::<bool>("auth")?;
    if authed.is_none() || !authed.unwrap() {
        return Ok(HttpResponse::SeeOther()
            .append_header(("Location", "/"))
            .body(()));
    }

    let mut zip_file =
        zip::ZipArchive::new(payload.zip_file.file.as_file()).map_err(ErrorUnsupportedMediaType)?;

    let files: Vec<String> = zip_file.file_names().map(|s| s.to_owned()).collect();

    for file_name in &files {
        let mut f = zip_file
            .by_name(file_name)
            .map_err(ErrorUnsupportedMediaType)?;
        if f.is_dir() {
            continue;
        }

        if f.name().ends_with(".md") && f.enclosed_name().is_some() {
            let mut md = String::new();
            f.read_to_string(&mut md)
                .map_err(ErrorUnsupportedMediaType)?;

            state
                .set_page(&f.enclosed_name().unwrap().to_string_lossy(), md)
                .await;
        }
    }

    Ok(UploadTemplate {
        name: state.name(),
        message: "Upload successful!",
    }
    .respond_to(&req))
}

#[derive(Template)]
#[template(path = "wiki.html")]
struct WikiTemplate<'a> {
    name: &'a str,
    title: &'a str,
    page: &'a Page,
}

#[get("/w{filepath:.*}")]
async fn wiki(
    req: HttpRequest,
    session: Session,
    state: Data<State>,
    path: actix_web::web::Path<String>,
) -> Result<impl Responder> {
    let authed = session.get::<bool>("auth")?;
    if authed.is_none() || !authed.unwrap() {
        return Ok(HttpResponse::SeeOther()
            .append_header(("Location", "/"))
            .body(()));
    }

    let mut trimmed_path = path.trim_start_matches("/");
    if trimmed_path.is_empty() {
        trimmed_path = "index.md";
    }

    let page = state.get_page(trimmed_path).await.unwrap_or_default();

    Ok(WikiTemplate {
        name: state.name(),
        title: "Wiki",
        page: &page,
    }
    .respond_to(&req))
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    let state = State::new();

    let master_key = state.master_key().await;

    HttpServer::new(move || {
        App::new()
            .wrap(
                SessionMiddleware::builder(CookieSessionStore::default(), master_key.clone())
                    .cookie_secure(false)
                    .build(),
            )
            .app_data(Data::new(state.clone()))
            .service(index)
            .service(login)
            .service(wiki)
            .service(favicon)
            .service(upload_page)
            .service(upload_file)
            .service(bootstrap_css)
            .service(bootstrap_js)
    })
    .bind(("0.0.0.0", 8080))?
    .run()
    .await
}

#[get("/favicon.svg")]
async fn favicon() -> impl Responder {
    HttpResponse::Ok().body(&include_bytes!("../assets/favicon.ico")[..])
}

#[get("/bootstrap.css")]
async fn bootstrap_css() -> impl Responder {
    HttpResponse::Ok().body(include_str!("../assets/bootstrap.css"))
}

#[get("/bootstrap.js")]
async fn bootstrap_js() -> impl Responder {
    HttpResponse::Ok().body(include_str!("../assets/bootstrap.js"))
}
