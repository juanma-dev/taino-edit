//! Axum server for the SSR demo. All the taino-edit-specific interest is in
//! `lib.rs`; this is the stock leptos_axum harness.

#[cfg(feature = "ssr")]
#[tokio::main]
async fn main() {
    use axum::Router;
    use leptos::prelude::*;
    use leptos_axum::{generate_route_list, LeptosRoutes};
    use ssr_leptos::{shell, App};

    let conf = get_configuration(None).expect("leptos configuration loads");
    let addr = conf.leptos_options.site_addr;
    let leptos_options = conf.leptos_options;
    let routes = generate_route_list(App);

    let app = Router::new()
        .leptos_routes(&leptos_options, routes, {
            let leptos_options = leptos_options.clone();
            move || shell(leptos_options.clone())
        })
        .fallback(leptos_axum::file_and_error_handler(shell))
        .with_state(leptos_options);

    println!("SSR demo listening on http://{addr}");
    let listener = tokio::net::TcpListener::bind(&addr)
        .await
        .expect("bind site address");
    axum::serve(listener, app.into_make_service())
        .await
        .expect("server runs");
}

/// The binary only exists in `ssr` builds; `cargo leptos` compiles the lib
/// side with `hydrate` instead.
#[cfg(not(feature = "ssr"))]
fn main() {
    eprintln!("Build with --features ssr (or run `cargo leptos watch`).");
}
