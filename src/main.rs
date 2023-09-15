use atspi::{
    connection::AccessibilityConnection,
    events::{AddAccessibleEvent, CacheEvents, LegacyAddAccessibleEvent, RemoveAccessibleEvent},
    proxy::{accessible::AccessibleProxy, application::ApplicationProxy},
    CacheItem, Event, Role,
};
use tokio_stream::StreamExt;
use zbus::{self, MessageType};

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

const APPLICATION_INTERFACE: &str = "org.a11y.atspi.Application";
const ACCESSIBLE_INTERFACE: &str = "org.a11y.atspi.Accessible";

async fn atspi_setup_connection() -> Result<AccessibilityConnection> {
    // Get a connection to the AT-SPI D-Bus service
    let atspi: AccessibilityConnection = AccessibilityConnection::open().await?;

    // Register for events with registryd & set match rules at the a11y bus
    // (if applicable)
    atspi.register_event::<AddAccessibleEvent>().await?;
    atspi.register_event::<LegacyAddAccessibleEvent>().await?;
    atspi.register_event::<RemoveAccessibleEvent>().await?;

    Ok(atspi)
}

#[tokio::main]
async fn main() -> Result<()> {
    let atspi = atspi_setup_connection().await?;
    let conn = atspi.connection();

    let mut raw_signals = zbus::MessageStream::from(conn)
        .filter(|msg| msg.is_ok() && msg.as_ref().unwrap().message_type() == MessageType::Signal);

    while let Some(msg) = raw_signals.next().await {
        match msg {
            Ok(msg) => {
                if let Ok(event) = Event::try_from(&*msg) {
                    match event {
                        Event::Cache(CacheEvents::Add(event)) => {
                            // print body signature
                            println!(
                                "AddAccessible DBus body signature: {}",
                                msg.body_signature().unwrap().as_str()
                            );

                            let AddAccessibleEvent { node_added, .. } = event.clone();
                            let CacheItem { app, .. } = node_added;

                            let bus_name = app.name;
                            let obj_path = app.path;
                            println!("Root object of Cache event bus_name: {bus_name}, obj_path: {obj_path}");

                            // Application Proxy for `app`:
                            if let Ok(application_proxy) =
                                zbus::ProxyBuilder::<ApplicationProxy>::new(conn)
                                    .interface(APPLICATION_INTERFACE)?
                                    .path(obj_path.as_str())?
                                    .destination(bus_name.as_str())?
                                    .build()
                                    .await
                            {
                                let toolkit_name = application_proxy
                                    .toolkit_name()
                                    .await
                                    .unwrap_or("Could not read toolkit property".to_string());
                                println!("toolkit: {toolkit_name}");
                            };

                            // AccessibleProxy for `app`:
                            if let Ok(accessible_proxy) =
                                zbus::ProxyBuilder::<AccessibleProxy>::new(conn)
                                    .interface(ACCESSIBLE_INTERFACE)?
                                    .path(obj_path.as_str())?
                                    .destination(bus_name.as_str())?
                                    .build()
                                    .await
                            {
                                let name: String = accessible_proxy
                                    .name()
                                    .await
                                    .unwrap_or("Could not read name property".to_string());
                                println!("name: {name}");

                                let description: String = accessible_proxy
                                    .description()
                                    .await
                                    .unwrap_or("Could not obtain a description.".to_string());
                                println!("description: {description}");

                                let role =
                                    accessible_proxy.get_role().await.unwrap_or(Role::Unknown);
                                println!("role: {role}");
                            };

                            // println!(": {:?}", event);
                        }

                        Event::Cache(CacheEvents::Remove(_event)) => {
                            println!(
                                "RemoveAccessible: DBus body signature: {}",
                                msg.body_signature().unwrap().as_str()
                            );

                            //  println!(": {:?}", event);
                        }

                        Event::Cache(CacheEvents::LegacyAdd(_event)) => {
                            println!(
                                "LegacyAddAccessible: DBus body signature: {}",
                                msg.body_signature().unwrap().as_str()
                            );

                            // println!(": {:?}", event);
                        }
                        _ => {} // We do not care about other events
                    }
                }
            }
            Err(e) => {
                println!("Error: {:?}", e);
            }
        }
    }

    Ok(())
}
