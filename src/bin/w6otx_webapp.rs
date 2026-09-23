use axum::http::StatusCode;
use axum::response::Html;
use axum::routing::{get, post};
use axum::{Json, Router};
use maud::{DOCTYPE, PreEscaped, html};
use serde::{Deserialize, Serialize};
use snmp::SyncSession;
use std::io::IsTerminal;
use std::str::FromStr;
use std::time::Duration;
use strum::IntoEnumIterator;
use tower_http::trace::{DefaultMakeSpan, DefaultOnResponse, TraceLayer};
use tracing::Level;
use w6otx::w6otx_snmp;

const VERSION: &str = env!("CARGO_PKG_VERSION");

const DEFAULT_SNMP_HOST: &str = "apc-rpdu:161";

#[derive(Debug, Serialize)]
struct OutletStatus {
    outlet: String,
    status: String,
}

#[derive(Debug, Serialize)]
struct SystemPowerStatus {
    statuses: Vec<OutletStatus>,
}

#[derive(Debug, Deserialize)]
struct ControlOutlet {
    outlet: String,
    command: String,
}

type HandlerError = (StatusCode, String);

fn new_session() -> Result<SyncSession, HandlerError> {
    let community = b"private";
    let timeout = Duration::from_secs(5);
    SyncSession::new(DEFAULT_SNMP_HOST, community, Some(timeout), 0)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))
}

async fn system_power_status() -> Result<Json<SystemPowerStatus>, HandlerError> {
    let mut session = new_session()?;
    let statuses = w6otx_snmp::Outlet::iter()
        .filter(|outlet| !outlet.to_string().starts_with("unused"))
        .map(|outlet| {
            let status = match w6otx_snmp::get_outlet_status(&mut session, outlet) {
                Ok(status) => status.to_string(),
                Err(_) => "? (failure)".into(),
            };
            OutletStatus {
                outlet: outlet.to_string(),
                status,
            }
        })
        .collect();
    Ok(Json(SystemPowerStatus { statuses }))
}

async fn control_outlet(
    Json(ControlOutlet { outlet, command }): Json<ControlOutlet>,
) -> Result<&'static str, HandlerError> {
    let bad_request = |e: strum::ParseError| (StatusCode::BAD_REQUEST, e.to_string());
    let outlet = w6otx_snmp::Outlet::from_str(&outlet).map_err(bad_request)?;
    let command = w6otx_snmp::OutletControlCommand::from_str(&command).map_err(bad_request)?;
    let mut session = new_session()?;
    match w6otx_snmp::control_outlet(&mut session, outlet, command) {
        Ok(_) => Ok("ok"),
        Err(_) => Ok("failed"),
    }
}

async fn root() -> Html<String> {
    Html(root_page())
}

#[tokio::main]
async fn main() -> std::io::Result<()> {
    tracing_subscriber::fmt()
        .with_max_level(Level::INFO)
        .with_ansi(std::io::stdout().is_terminal())
        .init();

    let app = Router::new()
        .route("/", get(root))
        .route("/system_power_status", get(system_power_status))
        .route("/control_outlet", post(control_outlet))
        .layer(
            TraceLayer::new_for_http()
                .make_span_with(DefaultMakeSpan::new().level(Level::INFO))
                .on_response(DefaultOnResponse::new().level(Level::INFO)),
        );

    let listener = tokio::net::TcpListener::bind("0.0.0.0:8080").await?;
    axum::serve(listener, app).await
}

fn root_page() -> String {
    html! {
      (DOCTYPE)
          html lang="en" {
              head {
                  meta name="viewport" content="width=device-width, initial-scale=1.0";
                  title { "W6OTX Power Status " }
                  style {
                      (PreEscaped(r#"
            table {
              border-collapse: collapse;
	      /*width: 50%;*/
              /*margin: 20px auto;*/
            }
            th, td {
              border: 1px solid #ddd;
              padding: 8px;
              text-align: left;
            }
            th {
              background-color: #f2f2f2;
            }
           .btn {
              padding: 5px 10px;
              margin-right: 5px;
              cursor: pointer;
            }
            .btn-on {
              background-color: #4CAF50;
              color: white;
            }
            .btn-off {
              background-color: #f44336;
              color: white;
            }
            .btn-bounce {
              background-color: #2196F3;
              color: white;
            }
          "#))
                  }
                  script {
                      (PreEscaped(r#"
        async function fetchStatus() {
            try {
                const response = await fetch('/system_power_status');
                const data = await response.json();
                updateTable(data.statuses);
            } catch (error) {
                console.error('Error fetching status:', error);
            }
        }

        function updateTable(statuses) {
            const statusBody = document.getElementById('statusBody');
            statusBody.innerHTML = '';
            statuses.forEach(status => {
                const row = document.createElement('tr');
                row.innerHTML = `
                    <td>${status.outlet}</td>
                    <td>${status.status}</td>
                    <td>
                        <button class="btn btn-on" onclick="sendCommand('${status.outlet}', 'immediate-on')">On</button>
                        <button class="btn btn-off" onclick="sendCommand('${status.outlet}', 'immediate-off')">Off</button>
                        <button class="btn btn-bounce" onclick="sendCommand('${status.outlet}', 'immediate-reboot')">Bounce</button>
                    </td>
                `;
                statusBody.appendChild(row);
            });
        }

        async function sendCommand(outlet, command) {
            try {
                const payload = { outlet, command };
                const response = await fetch('/control_outlet', {
                    method: 'POST',
                    headers: {
                        'Content-Type': 'application/json'
                    },
                    body: JSON.stringify(payload)
                });
                if (response.ok) {
                    fetchStatus();
                } else {
                    console.error('Failed to send command:', response.statusText);
                }
            } catch (error) {
                console.error('Error sending command:', error);
            }
        }

        setInterval(fetchStatus, 5000);
        fetchStatus();
        "#))
                  }

              }
              body {
                  h1 { "W6OTX Power Status" }
                  table #statusTable {
                      thead {
                          tr {
                              th { "Outlet" }
                              th { "Status" }
                              th { "Actions" }
                          }
                      }
                      tbody #statusBody {
                          (PreEscaped(r#"<!-- Status data will be inserted here -->"#))
                      }
                  }
                  hr;
                  p { "Version " (VERSION) }
              }
          }
  }.into_string()
}
