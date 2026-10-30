//! The installation's licence (DNK-77): verified offline at start, again every day, and when
//! the operator asks for a reload (`SIGHUP`), so a new file needs no restart.
//!
//! Only Studio reads it. Released rules keep being served by the Runtime whatever it says;
//! here it decides whether new versions and releases are made, whether releases reach
//! production, and which optional modules are offered.

use crate::error::ApiError;
use donka_licence::{Licence, LicenceError, Module, PublicKey, Status};
use donka_shared::clock::Clock;
use std::collections::BTreeSet;
use std::path::PathBuf;
use std::sync::{Arc, RwLock};
use std::time::Duration;

/// How often the licence file is read and verified again.
const CHECK_EVERY: Duration = Duration::from_secs(24 * 3600);

#[derive(Clone)]
pub struct Licensing {
    inner: Arc<Inner>,
}

struct Inner {
    file: Option<PathBuf>,
    /// The modules the installation turns on (`DONKA_MODULES`, DNK-74); all when `None`.
    enabled: Option<BTreeSet<Module>>,
    key: PublicKey,
    clock: Arc<dyn Clock>,
    /// What the file said when last read; `None` without a file (evaluation).
    verified: RwLock<Option<Result<Licence, LicenceError>>>,
}

impl Licensing {
    /// Reads and verifies `file` with `key` now. A file that cannot be trusted does not stop
    /// Studio: it is refused, named in the log, and Studio stays readable.
    pub fn new(
        file: Option<PathBuf>,
        enabled: Option<BTreeSet<Module>>,
        key: PublicKey,
        clock: Arc<dyn Clock>,
    ) -> Self {
        let licensing = Self {
            inner: Arc::new(Inner {
                file,
                enabled,
                key,
                clock,
                verified: RwLock::new(None),
            }),
        };
        licensing.reload();
        licensing
    }

    /// Reads the file again, verifies it, and logs what it means today.
    pub fn reload(&self) -> Status {
        let verified = self.inner.file.as_ref().map(|path| {
            std::fs::read_to_string(path)
                .map_err(|err| {
                    LicenceError::Unreadable(format!("cannot read {}: {err}", path.display()))
                })
                .and_then(|text| self.inner.key.verify(&text))
        });
        *self
            .inner
            .verified
            .write()
            .unwrap_or_else(|poisoned| poisoned.into_inner()) = verified;
        let status = self.status();
        self.log(&status);
        status
    }

    /// Where the installation stands today.
    pub fn status(&self) -> Status {
        let verified = self
            .inner
            .verified
            .read()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .clone();
        Status::on(verified, self.inner.clock.now().date_naive())
    }

    /// New versions and releases need a valid licence, or none at all (evaluation).
    pub fn require_changes(&self) -> Result<(), ApiError> {
        let status = self.status();
        if status.allows_changes() {
            Ok(())
        } else {
            Err(ApiError::LicenceInactive(status_name(&status)))
        }
    }

    /// Production needs a licence: evaluation stops at staging.
    pub fn require_production(&self) -> Result<(), ApiError> {
        if self.status().allows_production() {
            Ok(())
        } else {
            Err(ApiError::ProductionNeedsLicence)
        }
    }

    /// The module is part of this installation: the licence includes it and it is turned on.
    pub fn require(&self, module: Module) -> Result<(), ApiError> {
        if !self.status().includes(module) {
            Err(ApiError::ModuleNotLicensed(module))
        } else if !self.turned_on(module) {
            Err(ApiError::ModuleOff(module))
        } else {
            Ok(())
        }
    }

    pub fn includes(&self, module: Module) -> bool {
        self.status().includes(module) && self.turned_on(module)
    }

    /// The installation turns the module on (`DONKA_MODULES`), whatever the licence says.
    pub fn turned_on(&self, module: Module) -> bool {
        self.inner
            .enabled
            .as_ref()
            .is_none_or(|enabled| enabled.contains(&module))
    }

    fn log(&self, status: &Status) {
        let file = self
            .inner
            .file
            .as_ref()
            .map(|path| path.display().to_string());
        match status {
            Status::Evaluation => tracing::warn!(
                "no licence file (DONKA_LICENCE_FILE): Studio runs in evaluation, where releases \
                 cannot reach production"
            ),
            Status::Active { licence, days_left } => {
                let modules = module_names(licence);
                if status.expiring() {
                    tracing::warn!(
                        customer = %licence.customer,
                        until = %licence.valid_until,
                        days_left,
                        "the licence ends soon: ask Donka for the next one"
                    );
                } else {
                    tracing::info!(
                        customer = %licence.customer,
                        until = %licence.valid_until,
                        modules = %modules,
                        "licence verified"
                    );
                }
            }
            Status::NotYetValid { licence } => tracing::warn!(
                customer = %licence.customer,
                from = %licence.valid_from,
                "the licence is not valid yet: new versions and releases wait for its first day"
            ),
            Status::Expired { licence } => tracing::error!(
                customer = %licence.customer,
                until = %licence.valid_until,
                "the licence has expired: Studio stays readable and exportable, but refuses new \
                 versions and releases; released rules keep being served"
            ),
            Status::Refused { reason } => tracing::error!(
                file = file.as_deref().unwrap_or_default(),
                %reason,
                "licence file refused: Studio stays readable and exportable, but refuses new \
                 versions and releases"
            ),
        }
    }
}

/// The name an API client sees for a status.
pub fn status_name(status: &Status) -> &'static str {
    match status {
        Status::Evaluation => "evaluation",
        Status::Active { .. } => "active",
        Status::NotYetValid { .. } => "notYetValid",
        Status::Expired { .. } => "expired",
        Status::Refused { .. } => "refused",
    }
}

fn module_names(licence: &Licence) -> String {
    let names: Vec<_> = licence.modules.iter().map(|m| m.as_str()).collect();
    if names.is_empty() {
        "none".to_owned()
    } else {
        names.join(",")
    }
}

/// Verifies the licence file again every day, and at once on `SIGHUP`.
pub async fn watch(licensing: Licensing) {
    let mut daily = tokio::time::interval(CHECK_EVERY);
    // The first tick is immediate; Studio verified the file when it started.
    daily.tick().await;
    #[cfg(unix)]
    let mut hangup = match tokio::signal::unix::signal(tokio::signal::unix::SignalKind::hangup()) {
        Ok(signal) => Some(signal),
        Err(err) => {
            tracing::warn!(%err, "SIGHUP cannot reload the licence; it is checked daily");
            None
        }
    };
    loop {
        #[cfg(unix)]
        let reload_asked = async {
            match hangup.as_mut() {
                Some(signal) => {
                    signal.recv().await;
                }
                None => std::future::pending::<()>().await,
            }
        };
        #[cfg(not(unix))]
        let reload_asked = std::future::pending::<()>();
        tokio::select! {
            _ = daily.tick() => {}
            () = reload_asked => tracing::info!("reloading the licence file"),
        }
        licensing.reload();
    }
}
