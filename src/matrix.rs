use anyhow::Result;
use clap::Parser;
use futures_util::StreamExt;
use matrix_sdk::{
    Client,
    config::SyncSettings,
    ruma::{
        OwnedRoomId,
        api::client::{
            account::register::v3::Request as RegistrationRequest,
            account::register::RegistrationKind,
        },
    }, matrix_auth::{MatrixSession, MatrixSessionTokens}, SessionMeta
};
use matrix_sdk_ui::timeline::{RoomExt, PaginationOptions};
use std::sync::OnceLock;
use url::Url;


#[derive(Parser, Debug)]
struct Cli {
    /// The room id that we should listen for the,
    #[clap(value_parser)]
    room_id: OwnedRoomId,

    /// The user name that should be used for the login.
    #[clap(value_parser)]
    user_name: Option<String>,
    
    /// The password that should be used for the login.
    #[clap(value_parser)]
    password: Option<String>,

    /// The homeserver to connect to.
    #[clap(value_parser)]
    homeserver: Option<Url>,

    /// Set the proxy that should be used for the connection.
    #[clap(short, long)]
    proxy: Option<Url>,

    /// Enable verbose logging output.
    #[clap(short, long, action)]
    verbose: bool,
}

async fn login(cli: Cli) -> Result<(Client, Option<String>)> {
    // Note that when encryption is enabled, you should use a persistent store to be
    // able to restore the session with a working encryption setup.
    // See the `persist_session` example.
    let homeserver_url = cli.homeserver.as_ref()
        .map(|h| h.as_str())
        .unwrap_or("https://matrix.org");
    let mut builder = Client::builder().homeserver_url(homeserver_url);

    if let Some(proxy) = cli.proxy {
        builder = builder.proxy(proxy);
    }

    let client = builder.build().await?;

    let mut token = None;

    // If the `user_name` and `password` CLI arguments were provided, try to log in.
    if let (Some(ref un), Some(ref pw)) = (cli.user_name, cli.password) {
        client
            .matrix_auth()
            .login_username(un, pw)
            .initial_device_display_name("robrix-un-pw")
            .await?;
    } 
    // If not, attempt to register for a guest account.
    else {
        let mut request = RegistrationRequest::new();
        request.kind = RegistrationKind::Guest;
        let response = client
            .matrix_auth()
            .register(request)
            .await?;
        println!("Guest account registered: {:#?}", response);
        let access_token = response.access_token.expect("No access token provided by the server");
        token = Some(access_token.clone());
        let id = response.user_id;

        let login_types = client
            .matrix_auth()
            .get_login_types()
            .await?;
        println!("Login types: {:#?}", login_types);
    
        // let guest_client = Client::builder()
        //     .homeserver_url(homeserver_url)
        //     .build()
        //     .await?;


        println!("Currently logged in? {:?}", client.logged_in());

        let session = MatrixSession {
            meta: SessionMeta {
                user_id: id,
                device_id: response.device_id.clone().unwrap(),
            },
            tokens: MatrixSessionTokens {
                access_token: access_token.clone(),
                refresh_token: None,
            }
        };
        println!("Trying to restore session: {:#?}", session);
        client
            .matrix_auth()
            .restore_session(session)
            .await?;

        println!("Currently logged in? {:?}", client.logged_in());


        if false {
        // Log in with the guest account token.
        client
            .matrix_auth()
            .login_token(&access_token)
            .initial_device_display_name("robrix-guest")
            .await?;
        println!("Currently logged in? {:?}", client.logged_in());

        let whoami = client.whoami().await?;
        println!("whoami: {:#?}", whoami);
        }
    }


    Ok((client, token))
}

static TOKIO_RUNTIME: OnceLock<tokio::runtime::Runtime> = OnceLock::new();

pub fn start_matrix_tokio() -> Result<()> {
    // Save the tokio runtime in a static variable to ensure it isn't dropped.
    let rt = TOKIO_RUNTIME.get_or_init(|| tokio::runtime::Runtime::new().unwrap());
    rt.spawn(async_main());
    Ok(())
}

async fn async_main() -> Result<()> {
    tracing_subscriber::fmt::init();

    let cli = Cli::parse();
    let room_id = cli.room_id.clone();
    let (client, _token) = login(cli).await?;

    let sync_settings = SyncSettings::default();

    // Wait for the first sync response
    println!("Wait for the first sync");
    client.sync_once(sync_settings.clone()).await?;

    println!("Rooms: ");
    client.rooms().into_iter().for_each(|room| {
        println!("Room {:?}, id: {:?} ", room.name(), room.room_id());
    });

    // println!("Joining room: {:?}...", room_id);
    // let joined = client.join_room_by_id(&room_id).await?;
    // println!("Joined room: {:?}({:?})", joined.name(), joined.room_id());
        
    println!("Getting room ID: {:?}...", room_id);
    let room = client.get_room(&room_id).unwrap();
    println!("Got room: {:?} ({:?})", room.name(), room.room_id());
    
    // Get the timeline stream and listen to it.
    let timeline = room.timeline().await;
    timeline.paginate_backwards(PaginationOptions::single_request(u16::MAX)).await?;

    let (timeline_items, mut timeline_stream) = timeline.subscribe().await;

    println!("Initial timeline items: {timeline_items:#?}");
    tokio::spawn(async move {
        while let Some(diff) = timeline_stream.next().await {
            println!("Received a timeline diff: {diff:#?}");
        }
    });

    // Sync forever
    client.sync(sync_settings).await?;

    Ok(())
}