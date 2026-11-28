//! Re-export UI-facing DTOs from [`boson_backend`].

pub use boson_backend::{
    clamp_page_list_limit, AttentionKind, BosonPeopleLookup, BosonPoolProvider,
    DashboardChartPoint, DashboardChartSeries, DashboardStats, FailingTaskRow, GluonPoolPickRow,
    IdempotencyModeDto, JobActionFailure, JobActionOutcome, JobStatusDto, JobSummary,
    NeedsAttentionLine, QueueDetail, QueueStateDto, QueueSummary, RateLimitPolicyDto, RetriedJob,
    RetryPolicyDto, RunDetail, RunJobInfo, RunStatusDto, RunSummary, TaskArgumentRow,
    TaskConfigDto, TaskDetail, TaskSummary, UpdateTaskConfigRequest, JOBS_PAGE_SIZE,
    MAX_PAGE_LIST_LIMIT, RUNS_PAGE_SIZE, TASKS_PAGE_SIZE,
};
