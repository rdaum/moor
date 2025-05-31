use leptos::prelude::*;
use leptos::task::spawn_local;
use leptos_meta::{provide_meta_context, MetaTags, Stylesheet, Title};
use leptos_router::{
    components::{Route, Router, Routes},
    StaticSegment,
};

pub fn shell(options: LeptosOptions) -> impl IntoView {
    view! {
        <!DOCTYPE html>
        <html lang="en">
            <head>
                <meta charset="utf-8"/>
                <meta name="viewport" content="width=device-width, initial-scale=1"/>
                <AutoReload options=options.clone() />
                <HydrationScripts options/>
                <MetaTags/>
            </head>
            <body>
                <App/>
            </body>
        </html>
    }
}

#[component]
pub fn App() -> impl IntoView {
    // Provides context that manages stylesheets, titles, meta tags, etc.
    provide_meta_context();

    view! {
        // injects a stylesheet into the document <head>
        // id=leptos means cargo-leptos will hot-reload this stylesheet
        <Stylesheet id="leptos" href="/pkg/moor-leptos.css"/>

        // sets the document title
        <Title text="Welcome to Leptos"/>

        // content for this welcome page
        <Router>
            <main>
                <Routes fallback=|| "Page not found.".into_view()>
                    <Route path=StaticSegment("") view=HomePage/>
                </Routes>
            </main>
        </Router>
    }
}

/// Renders the home page of your application.
#[component]
fn HomePage() -> impl IntoView {
    // Creates a reactive value to update the button
    let count = RwSignal::new(0);
    let on_click = move |_| *count.write() += 1;
    let player_name = RwSignal::new("".to_string());
    let password = RwSignal::new("".to_string());
    view! {
        <h1>"Welcome to mooR!"</h1>
        <button on:click=on_click>"Click Me: " {count}</button>

        <input type="text" bind:value=player_name/>
        <input type="password" bind:value=password placeholder="Password"/>
        <button on:click=move |_| {
            let player_name = player_name.get();
            let password = password.get();
            spawn_local(async {
                perform_login(player_name, password).await;
            });
        }>
            "Add Todo"
        </button>
    }
}

#[server]
pub async fn perform_login(player_name: String, password: String) -> Result<(), ServerFnError> {
    use crate::establish_client_connection;
    use crate::ClientSession;
    use crate::Context;
    use moor_var::SYSTEM_OBJECT;
    use rpc_common::HostClientToDaemonMessage;
    use rpc_common::{DaemonToClientReply, ReplyResult};
    use std::sync::Arc;
    use tokio::sync::Mutex;
    use tracing::error;
    use tracing::info;

    if let Some(mut context) = take_context::<Context>() {
        info!(
            "Using state in server function with context, player_name: {player_name}, rpc_address: {}",
            context.rpc_address
        );
        // Let's try to make a connection to the RPC server and send a login request.
        let (client_id, mut rpc_client, client_token) = establish_client_connection(&context)
            .await
            .expect("Failed to connect to RPC server");

        info!("Successfully established connection with client ID: {client_id:?}, token: {client_token:?}");

        // Now we'll send an authentication request to the server using this client.
        let response = rpc_client
            .make_client_rpc_call(
                client_id,
                HostClientToDaemonMessage::LoginCommand(
                    client_token.clone(),
                    SYSTEM_OBJECT,
                    vec!["connect".to_string(), player_name, password],
                    false,
                ),
            )
            .await
            .expect("Unable to send login request to RPC server");

        info!("Received response from RPC server: {:?}", response);

        let ReplyResult::ClientSuccess(DaemonToClientReply::LoginResult(Some((
            auth_token,
            _connect_type,
            player,
        )))) = response
        else {
            error!("Login failed, unexpected response: {:?}", response);
            return Err(ServerFnError::new("Login failed"));
        };

        let client_session = ClientSession {
            context: context.clone(),
            player: SYSTEM_OBJECT,
            client_id,
            rpc_send_client: Arc::new(Mutex::new(rpc_client)),
            client_token: client_token.clone(),
        };
        provide_context(move || client_session);
    }
    Ok(())
}
