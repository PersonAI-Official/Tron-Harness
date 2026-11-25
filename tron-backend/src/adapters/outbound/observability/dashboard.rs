use super::*;
use std::collections::BTreeSet;
use sysinfo::{ProcessRefreshKind, ProcessesToUpdate, System};

impl DashboardRepository for TelemetryProjection {
    fn dashboard(&self, workspace: &str) -> Result<ObservabilityDashboard, ObservabilityError> {
        let q = TraceQuery {
            filters: Default::default(),
            category: None,
            sorts: Vec::new(),
            sort_by: Default::default(),
            sort_direction: Default::default(),
            workspace_code: workspace.into(),
            session_code: None,
            trace_id: None,
            current: 1,
            size: 1,
        };
        let runtime = self.runtime_metrics()?;
        let sampled_at = now();
        let (stats, sums) = self.statistics(&q, Some(sampled_at))?;
        let trace_count = stats.total;
        let failed_trace_count = *stats.classified.get("error").unwrap_or(&0);
        let running_trace_count = *stats.classified.get("running").unwrap_or(&0);
        let input_tokens = *sums.get("gen_ai.usage.input_tokens").unwrap_or(&0);
        let output_tokens = *sums.get("gen_ai.usage.output_tokens").unwrap_or(&0);
        let average_duration_ms = stats.average_duration_ms;
        let p95_duration_ms = stats.p95_duration_ms;
        let mut recent_requests = request_buckets(sampled_at);
        for bucket in stats.buckets {
            if let Some(target) = recent_requests
                .iter_mut()
                .find(|b| b.started_at == bucket.started_at)
            {
                target.requests = bucket.total;
                target.errors = *bucket.classified.get("error").unwrap_or(&0);
            }
        }
        let metric_point_count = if workspace.is_empty() {
            runtime.metric_point_count
        } else {
            blocking(async {
                self.store
                    .count_metric_points_by_attribute(
                        "tron.workspace.code".into(),
                        workspace.into(),
                    )
                    .await
                    .map_err(failure)
            })?
        };
        Ok(ObservabilityDashboard {
            sampled_at,
            trace_count,
            failed_trace_count,
            running_trace_count,
            input_tokens,
            output_tokens,
            average_duration_ms,
            p95_duration_ms,
            recent_requests,
            cpu_percent: runtime.cpu_percent,
            memory_bytes: runtime.memory_bytes,
            resource_sampled_at: runtime.sampled_at,
            metric_point_count,
            processes: service_processes(),
        })
    }
}
fn request_buckets(at: i64) -> Vec<RequestBucket> {
    let last_minute = at / 60_000 * 60_000;
    (0..15)
        .map(|i| RequestBucket {
            started_at: last_minute - (14 - i) * 60_000,
            requests: 0,
            errors: 0,
        })
        .collect()
}

fn service_processes() -> Vec<ServiceProcess> {
    let mut system = System::new();
    system.refresh_processes_specifics(
        ProcessesToUpdate::All,
        true,
        ProcessRefreshKind::nothing().with_memory().without_tasks(),
    );
    let root = sysinfo::Pid::from_u32(std::process::id());
    let mut owned = BTreeSet::from([root]);
    loop {
        let next = system
            .processes()
            .iter()
            .filter(|(_, p)| p.parent().is_some_and(|id| owned.contains(&id)))
            .map(|(id, _)| *id)
            .collect::<Vec<_>>();
        let before = owned.len();
        owned.extend(next);
        if before == owned.len() {
            break;
        }
    }
    owned
        .into_iter()
        .filter_map(|pid| {
            system.process(pid).map(|p| ServiceProcess {
                pid: pid.as_u32(),
                parent_pid: if pid == root {
                    None
                } else {
                    p.parent().map(|id| id.as_u32())
                },
                name: if pid == root {
                    "tron-backend".into()
                } else {
                    p.name().to_string_lossy().into_owned()
                },
                memory_bytes: p.memory(),
                started_at: (p.start_time().min(i64::MAX as u64 / 1000) * 1000) as i64,
            })
        })
        .collect()
}
