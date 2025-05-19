use maud::{DOCTYPE, PreEscaped, html};
use snmp::SyncSession;
use std::str::FromStr;
use std::time::Duration;
use strum::IntoEnumIterator;
use tide::prelude::*;
use tide::{Request, Response, StatusCode};
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

async fn system_power_status(_: Request<()>) -> tide::Result {
    let community = b"private";
    let timeout = Duration::from_secs(5);
    let mut session = SyncSession::new(DEFAULT_SNMP_HOST, community, Some(timeout), 0)?;
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
    let system_power_status = SystemPowerStatus { statuses };
    let json = serde_json::to_string(&system_power_status)?;
    let response = Response::builder(StatusCode::Ok)
        .body(json)
        .content_type("application/json")
        .build();
    Ok(response)
}

async fn control_outlet(mut request: Request<()>) -> tide::Result {
    let ControlOutlet { outlet, command } = request.body_json().await?;
    let outlet = w6otx_snmp::Outlet::from_str(outlet.as_ref())?;
    let command = w6otx_snmp::OutletControlCommand::from_str(command.as_ref())?;
    let community = b"private";
    let timeout = Duration::from_secs(5);
    let mut session = SyncSession::new(DEFAULT_SNMP_HOST, community, Some(timeout), 0)?;
    match w6otx_snmp::control_outlet(&mut session, outlet, command) {
        Ok(_) => Ok("ok".into()),
        Err(_) => Ok("failed".into()),
    }
}

async fn root(_: Request<()>) -> tide::Result {
    let response = Response::builder(StatusCode::Ok)
        .body(root_page())
        .content_type("text/html")
        .build();
    Ok(response)
}

#[async_std::main]
async fn main() -> tide::Result<()> {
    femme::start();

    let mut app = tide::new();
    app.with(tide::log::LogMiddleware::new());

    app.at("/").get(root);
    app.at("/system_power_status").get(system_power_status);
    app.at("/control_outlet").post(control_outlet);

    app.listen("0.0.0.0:8080").await?;

    Ok(())
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
