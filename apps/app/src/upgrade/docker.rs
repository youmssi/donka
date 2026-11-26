//! The Docker Engine API, through the socket `donka-admin upgrade` is started with: its own
//! container, the installation's containers, pulls, and short helper containers that do the
//! work a shell would (pg_dump, tar, docker compose).

use bollard::auth::DockerCredentials;
use bollard::exec::StartExecResults;
use bollard::models::{ContainerCreateBody, ExecConfig, HostConfig, NetworkConnectRequest};
use bollard::query_parameters::{
    CreateContainerOptionsBuilder, CreateImageOptionsBuilder, DownloadFromContainerOptionsBuilder,
    ListContainersOptionsBuilder, LogsOptionsBuilder, RemoveContainerOptionsBuilder,
    TagImageOptionsBuilder, WaitContainerOptions,
};
use bollard::Docker;
use futures_util::StreamExt as _;
use std::collections::HashMap;
use std::io::Read as _;

/// The Compose project `docker-compose.yml` names (`name: donka`).
pub const PROJECT: &str = "donka";
/// Where the installation folder is mounted in the upgrade's own container.
pub const INSTALL: &str = "/install";

pub struct Engine {
    docker: Docker,
    /// This container: `donka-admin upgrade` itself.
    pub own_id: String,
    /// The installation folder on the host, which helper containers mount.
    pub host_dir: String,
    /// The user the upgrade runs as, so files it writes belong to the operator.
    pub user: String,
}

/// One of the installation's containers.
pub struct Container {
    pub service: String,
    pub id: String,
    pub image: String,
    pub image_id: String,
    pub running: bool,
    /// As `docker ps` says it, e.g. `Up 3 hours (healthy)`.
    pub status: String,
}

/// A helper container: an image, a command, its mounts and environment.
#[derive(Default)]
pub struct Helper {
    pub image: String,
    pub cmd: Vec<String>,
    pub binds: Vec<String>,
    pub env: Vec<String>,
    pub user: Option<String>,
    pub working_dir: Option<String>,
    /// Joins the installation's network, to reach `postgres`, `app` and `runtime`.
    pub network: bool,
}

impl Engine {
    pub async fn connect() -> Result<Self, String> {
        let docker = Docker::connect_with_socket_defaults().map_err(|err| {
            format!("no Docker socket ({err}): start donka-admin with -v /var/run/docker.sock:/var/run/docker.sock")
        })?;
        docker
            .ping()
            .await
            .map_err(|err| format!("Docker does not answer on its socket: {err}"))?;
        let own_id = std::env::var("HOSTNAME")
            .map_err(|_| "HOSTNAME is not set: run donka-admin in its container".to_owned())?;
        let own = docker
            .inspect_container(&own_id, None)
            .await
            .map_err(|err| format!("cannot inspect the upgrade's own container: {err}"))?;
        let host_dir = own
            .mounts
            .unwrap_or_default()
            .into_iter()
            .find(|mount| mount.destination.as_deref() == Some(INSTALL))
            .and_then(|mount| mount.source)
            .ok_or(
                "the installation folder is not mounted at /install: add -v \"$PWD:/install\"",
            )?;
        let user = own
            .config
            .and_then(|config| config.user)
            .filter(|user| !user.is_empty())
            .unwrap_or_else(|| "0".to_owned());
        Ok(Self {
            docker,
            own_id: own.id.unwrap_or(own_id),
            host_dir,
            user,
        })
    }

    /// The registries Docker talks plain HTTP to (its `insecure-registries`): the upgrade does
    /// the same, and nowhere else.
    pub async fn insecure_registries(&self) -> Result<Vec<String>, String> {
        let info = self.docker.info().await.map_err(|err| err.to_string())?;
        Ok(info
            .registry_config
            .and_then(|config| config.index_configs)
            .unwrap_or_default()
            .into_iter()
            .filter(|(_, index)| index.secure == Some(false))
            .map(|(name, _)| name)
            .collect())
    }

    /// Joins the installation's network, so its services answer by name.
    pub async fn join_network(&self) -> Result<(), String> {
        let network = format!("{PROJECT}_default");
        let attached = self
            .docker
            .inspect_container(&self.own_id, None)
            .await
            .map_err(|err| err.to_string())?
            .network_settings
            .and_then(|settings| settings.networks)
            .is_some_and(|networks| networks.contains_key(&network));
        if attached {
            return Ok(());
        }
        self.docker
            .connect_network(
                &network,
                NetworkConnectRequest {
                    container: self.own_id.clone(),
                    endpoint_config: None,
                },
            )
            .await
            .map_err(|err| format!("cannot join {network}: is the installation running? ({err})"))
    }

    /// The image ID a service's container runs, if it exists.
    pub async fn service_image(&self, service: &str) -> Result<Option<String>, String> {
        let filters = HashMap::from([(
            "label",
            vec![
                format!("com.docker.compose.project={PROJECT}"),
                format!("com.docker.compose.service={service}"),
            ],
        )]);
        let options = ListContainersOptionsBuilder::new()
            .all(true)
            .filters(&filters)
            .build();
        let containers = self
            .docker
            .list_containers(Some(options))
            .await
            .map_err(|err| err.to_string())?;
        Ok(containers
            .into_iter()
            .find_map(|container| container.image_id))
    }

    /// The local image ID of `image`, if Docker has it.
    pub async fn image_id(&self, image: &str) -> Result<Option<String>, String> {
        match self.docker.inspect_image(image).await {
            Ok(inspect) => Ok(inspect.id),
            Err(bollard::errors::Error::DockerResponseServerError {
                status_code: 404, ..
            }) => Ok(None),
            Err(err) => Err(err.to_string()),
        }
    }

    /// Pulls `name@digest` (what was verified, not whatever the tag points to by then) and
    /// tags it `name:tag`.
    pub async fn pull_verified(
        &self,
        name: &str,
        digest: &str,
        tag: &str,
        credentials: Option<(String, String)>,
    ) -> Result<(), String> {
        let reference = format!("{name}@{digest}");
        let options = CreateImageOptionsBuilder::new()
            .from_image(&reference)
            .build();
        let credentials = credentials.map(|(username, password)| DockerCredentials {
            username: Some(username),
            password: Some(password),
            ..Default::default()
        });
        let mut pull = self.docker.create_image(Some(options), None, credentials);
        while let Some(progress) = pull.next().await {
            progress.map_err(|err| format!("cannot pull {reference}: {err}"))?;
        }
        let options = TagImageOptionsBuilder::new().repo(name).tag(tag).build();
        self.docker
            .tag_image(&reference, Some(options))
            .await
            .map_err(|err| format!("cannot tag {reference} as {name}:{tag}: {err}"))
    }

    /// Pulls a tool image the upgrade runs (not one of Donka's, which are verified first).
    pub async fn pull_tag(&self, name: &str, tag: &str) -> Result<(), String> {
        let options = CreateImageOptionsBuilder::new()
            .from_image(name)
            .tag(tag)
            .build();
        let mut pull = self.docker.create_image(Some(options), None, None);
        while let Some(progress) = pull.next().await {
            progress.map_err(|err| format!("cannot pull {name}:{tag}: {err}"))?;
        }
        Ok(())
    }

    /// The contents of the file at `path` in `image`, read without running the image.
    pub async fn read_file(&self, image: &str, path: &str) -> Result<Vec<u8>, String> {
        let created = self
            .docker
            .create_container(
                None::<bollard::query_parameters::CreateContainerOptions>,
                ContainerCreateBody {
                    image: Some(image.to_owned()),
                    ..Default::default()
                },
            )
            .await
            .map_err(|err| format!("cannot open {image}: {err}"))?;
        let options = DownloadFromContainerOptionsBuilder::new()
            .path(path)
            .build();
        let mut archive = Vec::new();
        let mut download = self
            .docker
            .download_from_container(&created.id, Some(options));
        let mut failure = None;
        while let Some(chunk) = download.next().await {
            match chunk {
                Ok(bytes) => archive.extend_from_slice(&bytes),
                Err(err) => {
                    failure = Some(format!("{image} has no {path}: {err}"));
                    break;
                }
            }
        }
        self.remove(&created.id).await;
        if let Some(failure) = failure {
            return Err(failure);
        }
        let mut entries = tar::Archive::new(archive.as_slice());
        let mut entry = entries
            .entries()
            .and_then(|mut entries| entries.next().transpose())
            .map_err(|err| err.to_string())?
            .ok_or_else(|| format!("{image} has no {path}"))?;
        let mut content = Vec::new();
        entry
            .read_to_end(&mut content)
            .map_err(|err| err.to_string())?;
        Ok(content)
    }

    /// Runs a helper container to its end; fails with the end of its output when it fails.
    pub async fn run(&self, helper: Helper) -> Result<(), String> {
        let name = format!(
            "{PROJECT}-upgrade-{}",
            helper.cmd.first().map_or("helper", String::as_str)
        );
        let host_config = HostConfig {
            binds: Some(helper.binds),
            network_mode: helper.network.then(|| format!("{PROJECT}_default")),
            ..Default::default()
        };
        let options = CreateContainerOptionsBuilder::new()
            .name(&format!("{name}-{}", std::process::id()))
            .build();
        let created = self
            .docker
            .create_container(
                Some(options),
                ContainerCreateBody {
                    image: Some(helper.image.clone()),
                    cmd: Some(helper.cmd.clone()),
                    env: Some(helper.env),
                    user: helper.user,
                    working_dir: helper.working_dir,
                    host_config: Some(host_config),
                    ..Default::default()
                },
            )
            .await
            .map_err(|err| format!("cannot create a {} container: {err}", helper.image))?;
        let outcome = self.wait(&created.id).await;
        let output = self.output(&created.id).await;
        self.remove(&created.id).await;
        match outcome {
            Ok(0) => Ok(()),
            Ok(code) => Err(format!(
                "`{}` exited with {code}: {}",
                helper.cmd.join(" "),
                output.trim()
            )),
            Err(err) => Err(err),
        }
    }

    /// Starts a helper container and leaves it: Docker removes it when it ends. For the last
    /// step of an unattended upgrade, which recreates the upgrade's own container.
    pub async fn start_detached(&self, helper: Helper) -> Result<(), String> {
        let created = self
            .docker
            .create_container(
                None::<bollard::query_parameters::CreateContainerOptions>,
                ContainerCreateBody {
                    image: Some(helper.image.clone()),
                    cmd: Some(helper.cmd),
                    user: helper.user,
                    working_dir: helper.working_dir,
                    host_config: Some(HostConfig {
                        binds: Some(helper.binds),
                        auto_remove: Some(true),
                        ..Default::default()
                    }),
                    ..Default::default()
                },
            )
            .await
            .map_err(|err| format!("cannot create a {} container: {err}", helper.image))?;
        self.docker
            .start_container(
                &created.id,
                None::<bollard::query_parameters::StartContainerOptions>,
            )
            .await
            .map_err(|err| err.to_string())
    }

    async fn wait(&self, id: &str) -> Result<i64, String> {
        self.docker
            .start_container(id, None::<bollard::query_parameters::StartContainerOptions>)
            .await
            .map_err(|err| err.to_string())?;
        let mut wait = self.docker.wait_container(id, None::<WaitContainerOptions>);
        let mut code = 0;
        while let Some(status) = wait.next().await {
            match status {
                Ok(status) => code = status.status_code,
                // A non-zero exit arrives as an error carrying the code.
                Err(bollard::errors::Error::DockerContainerWaitError { code: exit, .. }) => {
                    code = exit
                }
                Err(err) => return Err(err.to_string()),
            }
        }
        Ok(code)
    }

    /// The installation's containers, running or not (`donka-admin support-bundle`).
    pub async fn containers(&self) -> Result<Vec<Container>, String> {
        let filters = HashMap::from([(
            "label",
            vec![format!("com.docker.compose.project={PROJECT}")],
        )]);
        let options = ListContainersOptionsBuilder::new()
            .all(true)
            .filters(&filters)
            .build();
        let mut containers: Vec<Container> = self
            .docker
            .list_containers(Some(options))
            .await
            .map_err(|err| err.to_string())?
            .into_iter()
            .map(|summary| {
                let label = |key: &str| {
                    summary
                        .labels
                        .as_ref()
                        .and_then(|labels| labels.get(key).cloned())
                        .unwrap_or_default()
                };
                Container {
                    service: label("com.docker.compose.service"),
                    id: summary.id.clone().unwrap_or_default(),
                    image: summary.image.clone().unwrap_or_default(),
                    image_id: summary.image_id.clone().unwrap_or_default(),
                    running: summary.state.as_ref().map(ToString::to_string).as_deref()
                        == Some("running"),
                    status: summary.status.clone().unwrap_or_default(),
                }
            })
            .collect();
        containers.sort_by(|a, b| a.service.cmp(&b.service));
        Ok(containers)
    }

    /// What a container wrote in the last `since_seconds`, at most `tail` lines.
    pub async fn logs(&self, id: &str, since_seconds: i64, tail: usize) -> Result<String, String> {
        let since = chrono::Utc::now().timestamp() - since_seconds;
        let options = LogsOptionsBuilder::new()
            .stdout(true)
            .stderr(true)
            .timestamps(true)
            .since(i32::try_from(since).unwrap_or(0))
            .tail(&tail.to_string())
            .build();
        let mut logs = self.docker.logs(id, Some(options));
        let mut text = String::new();
        while let Some(line) = logs.next().await {
            text.push_str(&line.map_err(|err| err.to_string())?.to_string());
        }
        Ok(text)
    }

    /// Runs `cmd` in a running container and returns what it wrote.
    pub async fn exec(&self, id: &str, cmd: &[&str]) -> Result<String, String> {
        let exec = self
            .docker
            .create_exec(
                id,
                ExecConfig {
                    attach_stdout: Some(true),
                    attach_stderr: Some(true),
                    cmd: Some(cmd.iter().map(|part| (*part).to_owned()).collect()),
                    ..Default::default()
                },
            )
            .await
            .map_err(|err| err.to_string())?;
        let mut text = String::new();
        if let StartExecResults::Attached { mut output, .. } = self
            .docker
            .start_exec(&exec.id, None)
            .await
            .map_err(|err| err.to_string())?
        {
            while let Some(chunk) = output.next().await {
                text.push_str(&chunk.map_err(|err| err.to_string())?.to_string());
            }
        }
        Ok(text)
    }

    /// The last lines a container wrote.
    async fn output(&self, id: &str) -> String {
        let options = LogsOptionsBuilder::new()
            .stdout(true)
            .stderr(true)
            .tail("20")
            .build();
        let mut logs = self.docker.logs(id, Some(options));
        let mut text = String::new();
        while let Some(Ok(line)) = logs.next().await {
            text.push_str(&line.to_string());
        }
        text
    }

    async fn remove(&self, id: &str) {
        let options = RemoveContainerOptionsBuilder::new().force(true).build();
        let _ = self.docker.remove_container(id, Some(options)).await;
    }
}
