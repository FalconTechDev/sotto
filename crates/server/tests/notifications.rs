//! Database-backed checks for durable lifecycle notice identity and delivery.
//!
//! These tests are opt-in because they create rows in a disposable local Postgres database.

use async_trait::async_trait;
use axum::body::{to_bytes, Body};
use axum::http::{Request, StatusCode};
use axum::Router;
use sqlx::postgres::PgConnectOptions;
use sqlx::{PgPool, Row};
use std::str::FromStr;
use std::sync::OnceLock;
use tokio::sync::Mutex;
use tower::ServiceExt;

use sotto_server::cloud_coverage::{evaluate, ConfirmedPaidInterval, PersonCoverage};
use sotto_server::cloud_coverage_store::{self, CoverageProjection};
use sotto_server::config::DeploymentMode;
use sotto_server::db;
use sotto_server::notifications::{
    self, DeliveryOutcome, NoticeChannel, NoticeContent, NoticeError, NoticeIntent, NoticeKind,
    NoticeSender,
};
use sotto_server::state::AppState;

fn notification_test_lock() -> &'static Mutex<()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
}

async fn pool_or_skip() -> Option<PgPool> {
    if std::env::var("SOTTO_RUN_DB_TESTS").as_deref() != Ok("1") {
        eprintln!("skipping notification tests: set SOTTO_RUN_DB_TESTS=1");
        return None;
    }
    let url = std::env::var("DATABASE_URL").expect("DATABASE_URL for opted-in database tests");
    let options = PgConnectOptions::from_str(&url).expect("parse DATABASE_URL");
    assert!(
        matches!(options.get_host(), "localhost" | "127.0.0.1" | "::1"),
        "refusing notification tests against non-local host: {}",
        options.get_host()
    );
    let pool = db::connect(&url).await.expect("connect to test database");
    db::migrate(&pool).await.expect("apply migrations");
    Some(pool)
}

async fn seed_user(pool: &PgPool, user_id: &str) {
    sqlx::query(
        "INSERT INTO users (id, oauth_provider, oauth_subject) VALUES ($1, 'notification-test', $1) \
         ON CONFLICT (id) DO NOTHING",
    )
    .bind(user_id)
    .execute(pool)
    .await
    .expect("seed notification user");
}

async fn publish_coverage(
    pool: &PgPool,
    user_id: &str,
    expected_revision: Option<i64>,
    operation_id: &str,
    paid_intervals: Vec<ConfirmedPaidInterval>,
) -> i64 {
    let mut tx = pool.begin().await.expect("begin coverage publication");
    let receipt = cloud_coverage_store::publish(
        &mut tx,
        user_id,
        expected_revision,
        operation_id,
        &format!("evidence-{operation_id}"),
        &CoverageProjection::Complete { paid_intervals },
    )
    .await
    .expect("publish complete coverage");
    tx.commit().await.expect("commit coverage publication");
    receipt.revision
}

fn eligibility_for(
    coverage: PersonCoverage,
    now_epoch: i64,
) -> sotto_server::person_eligibility::EligibilityView {
    let decision = evaluate(&coverage, now_epoch).expect("evaluate coverage");
    sotto_server::person_eligibility::evaluate_input(
        &sotto_server::person_eligibility::EligibilityInput {
            deployment_mode: DeploymentMode::Cloud,
            billing_available: false,
            account_initialized: true,
            personal_billing_state: None,
            coverage: sotto_server::person_eligibility::CoverageInput::Decision(decision),
        },
    )
}

async fn cleanup(pool: &PgPool, user_id: &str) {
    sqlx::query("DELETE FROM cloud_notice_outbox WHERE recipient_user_id=$1")
        .bind(user_id)
        .execute(pool)
        .await
        .expect("clean notification outbox");
    sqlx::query("DELETE FROM cloud_verified_contacts WHERE user_id=$1")
        .bind(user_id)
        .execute(pool)
        .await
        .expect("clean notification contacts");
    sqlx::query("DELETE FROM cloud_coverage_heads WHERE beneficiary_id=$1")
        .bind(user_id)
        .execute(pool)
        .await
        .expect("clean coverage head");
    sqlx::query("DELETE FROM cloud_coverage_revision_facts WHERE beneficiary_id=$1")
        .bind(user_id)
        .execute(pool)
        .await
        .expect("clean coverage facts");
    sqlx::query("DELETE FROM cloud_coverage_revisions WHERE beneficiary_id=$1")
        .bind(user_id)
        .execute(pool)
        .await
        .expect("clean coverage revisions");
    sqlx::query("DELETE FROM users WHERE id=$1")
        .bind(user_id)
        .execute(pool)
        .await
        .expect("clean notification user");
}

fn content() -> NoticeContent {
    NoticeContent {
        title: "Renewal failed".into(),
        detail: "Recovery is available until the displayed deadline.".into(),
        effective_at_epoch: Some(1_800_000_000),
        deadline_epoch: Some(1_800_100_000),
        amount_pence: Some(299),
    }
}

fn in_app_intent(user_id: &str, event_key: &str, due_at_epoch: i64) -> NoticeIntent {
    NoticeIntent {
        recipient_user_id: user_id.into(),
        event_key: event_key.into(),
        policy_key: "renewal-failure-v1".into(),
        kind: NoticeKind::FailedRenewal,
        channel: NoticeChannel::InApp,
        contact_id: None,
        due_at_epoch,
        content: content(),
    }
}

#[tokio::test]
async fn export_window_notice_tracks_the_complete_coverage_episode() {
    let _guard = notification_test_lock().lock().await;
    let Some(pool) = pool_or_skip().await else {
        return;
    };
    const USER_ID: &str = "notification-test-export-episode";
    cleanup(&pool, USER_ID).await;
    seed_user(&pool, USER_ID).await;

    let now_epoch: i64 = sqlx::query_scalar("SELECT extract(epoch FROM now())::bigint")
        .fetch_one(&pool)
        .await
        .expect("read database time");
    let first_end = now_epoch - 18 * 24 * 60 * 60;
    let latest_end = now_epoch - 10 * 24 * 60 * 60;
    let intervals = vec![
        ConfirmedPaidInterval {
            coverage_id: "personal-term-old".into(),
            source_id: "personal-source".into(),
            starts_at: first_end - 30 * 24 * 60 * 60,
            paid_until: first_end,
            failed_renewal_id: None,
        },
        ConfirmedPaidInterval {
            coverage_id: "sponsored-term-latest".into(),
            source_id: "sponsored-source".into(),
            starts_at: latest_end - 30 * 24 * 60 * 60,
            paid_until: latest_end,
            failed_renewal_id: None,
        },
    ];
    let revision = publish_coverage(
        &pool,
        USER_ID,
        None,
        "export-episode-ended",
        intervals.clone(),
    )
    .await;
    let coverage = PersonCoverage {
        beneficiary_id: USER_ID.into(),
        paid_intervals: intervals,
    };
    let view = eligibility_for(coverage.clone(), now_epoch);
    assert_eq!(
        view.state,
        sotto_server::person_eligibility::EligibilityState::ExportOnly
    );
    let deadline = latest_end + sotto_server::cloud_coverage::EXPORT_WINDOW_SECONDS;

    assert!(notifications::reconcile_export_deadline_notice(
        &pool,
        USER_ID,
        &view,
        Some(revision),
        now_epoch
    )
    .await
    .expect("record export notice"));
    assert!(notifications::reconcile_export_deadline_notice(
        &pool,
        USER_ID,
        &view,
        Some(revision),
        now_epoch + 1
    )
    .await
    .expect("replay export notice"));

    let notices = notifications::list_for_user(&pool, USER_ID)
        .await
        .expect("list export notice");
    assert_eq!(notices.len(), 1);
    assert_eq!(notices[0].kind, "export_deadline");
    assert_eq!(notices[0].content.deadline_epoch, Some(deadline));
    assert_eq!(notices[0].content.effective_at_epoch, Some(latest_end));
    assert_eq!(notices[0].due_at_epoch, latest_end);

    let live_until = now_epoch + 10 * 24 * 60 * 60;
    let mut extended_intervals = coverage.paid_intervals.clone();
    extended_intervals.push(ConfirmedPaidInterval {
        coverage_id: "transfer-term-live".into(),
        source_id: "transferred-source".into(),
        starts_at: now_epoch - 24 * 60 * 60,
        paid_until: live_until,
        failed_renewal_id: None,
    });
    let next_revision = publish_coverage(
        &pool,
        USER_ID,
        Some(revision),
        "export-episode-extended",
        extended_intervals.clone(),
    )
    .await;
    let paid_view = eligibility_for(
        PersonCoverage {
            beneficiary_id: USER_ID.into(),
            paid_intervals: extended_intervals,
        },
        now_epoch,
    );
    assert_eq!(
        paid_view.state,
        sotto_server::person_eligibility::EligibilityState::Paid
    );

    // A stale view from before the new source was published must not recreate the old notice.
    assert!(!notifications::reconcile_export_deadline_notice(
        &pool,
        USER_ID,
        &view,
        Some(revision),
        now_epoch
    )
    .await
    .expect("reject stale coverage revision"));
    assert!(notifications::reconcile_export_deadline_notice(
        &pool,
        USER_ID,
        &paid_view,
        Some(next_revision),
        now_epoch
    )
    .await
    .expect("supersede ended notice while covered"));
    assert!(notifications::list_for_user(&pool, USER_ID)
        .await
        .expect("list notices while paid")
        .is_empty());

    let future_view = eligibility_for(
        PersonCoverage {
            beneficiary_id: USER_ID.into(),
            paid_intervals: paid_view_coverage(&coverage, live_until),
        },
        live_until + 1,
    );
    assert_eq!(
        future_view.state,
        sotto_server::person_eligibility::EligibilityState::ExportOnly
    );
    assert!(notifications::reconcile_export_deadline_notice(
        &pool,
        USER_ID,
        &future_view,
        Some(next_revision),
        live_until + 1,
    )
    .await
    .expect("record the later export episode"));
    let later_notice: (String, i64, i64) = sqlx::query_as(
        "SELECT status, extract(epoch FROM due_at)::bigint, \
                (payload->>'deadline_epoch')::bigint \
         FROM cloud_notice_outbox WHERE recipient_user_id=$1 AND kind='export_deadline' \
           AND policy_key='cloud-export-deadline-v1' AND status='pending'",
    )
    .bind(USER_ID)
    .fetch_one(&pool)
    .await
    .expect("load later scheduled export notice");
    assert_eq!(later_notice.0, "pending");
    assert_eq!(later_notice.1, live_until);
    assert_eq!(later_notice.2, live_until + 30 * 24 * 60 * 60);
    assert!(notifications::list_for_user(&pool, USER_ID)
        .await
        .expect("future export notice is hidden")
        .is_empty());

    cleanup(&pool, USER_ID).await;
}

#[tokio::test]
async fn eligibility_route_records_export_notice_without_vault_unlock() {
    let _guard = notification_test_lock().lock().await;
    let Some(pool) = pool_or_skip().await else {
        return;
    };
    const USER_ID: &str = "notification-test-export-route";
    cleanup(&pool, USER_ID).await;
    seed_user(&pool, USER_ID).await;
    sqlx::query("UPDATE users SET public_key=$2 WHERE id=$1")
        .bind(USER_ID)
        .bind(vec![7_u8; 32])
        .execute(&pool)
        .await
        .expect("mark account as initialised");

    let now_epoch: i64 = sqlx::query_scalar("SELECT extract(epoch FROM now())::bigint")
        .fetch_one(&pool)
        .await
        .expect("read database time");
    let paid_until = now_epoch - 3 * 24 * 60 * 60;
    let deadline = paid_until + sotto_server::cloud_coverage::EXPORT_WINDOW_SECONDS;
    publish_coverage(
        &pool,
        USER_ID,
        None,
        "export-route-episode",
        vec![ConfirmedPaidInterval {
            coverage_id: "export-route-term".into(),
            source_id: "export-route-source".into(),
            starts_at: paid_until - 30 * 24 * 60 * 60,
            paid_until,
            failed_renewal_id: None,
        }],
    )
    .await;

    let token = sotto_server::auth::session::issue(&pool, USER_ID)
        .await
        .expect("issue test session");
    let app = Router::new()
        .merge(sotto_server::person_eligibility::router())
        .with_state(AppState {
            pool: pool.clone(),
            deployment_mode: DeploymentMode::Cloud,
            oauth: None,
            oauth_config: None,
            billing: None,
            telemetry_ingest: false,
            organisation_deletion_enabled: false,
            organisation_deletion_retention_days:
                sotto_server::config::DEFAULT_ORGANISATION_DELETION_RETENTION_DAYS,
            organisation_deletion_metrics_token: None,
            organisation_deletion_operator_token: None,
            billing_transfer_operator_token: None,
            cloud_action_enforcement_enabled: false,
            machine_eligibility_enforcement_enabled: false,
            billing_transfers_enabled: false,
            billing_transfer_execution_enabled: false,
            billing_transfer_setup_enabled: false,
        });
    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/account/eligibility")
                .header("authorization", format!("Bearer {token}"))
                .body(Body::empty())
                .expect("build eligibility request"),
        )
        .await
        .expect("get eligibility");
    assert_eq!(response.status(), StatusCode::OK);
    let body = to_bytes(response.into_body(), 64 * 1024)
        .await
        .expect("read eligibility response");
    let eligibility: serde_json::Value =
        serde_json::from_slice(&body).expect("decode eligibility response");
    assert_eq!(eligibility["state"], "export_only");
    assert_eq!(eligibility["export_until_epoch"], deadline);
    assert_eq!(eligibility["actions"]["export"], true);

    let notices = notifications::list_for_user(&pool, USER_ID)
        .await
        .expect("list notices after eligibility read");
    assert_eq!(notices.len(), 1);
    assert_eq!(notices[0].kind, "export_deadline");
    assert_eq!(notices[0].content.deadline_epoch, Some(deadline));

    cleanup(&pool, USER_ID).await;
}

fn paid_view_coverage(previous: &PersonCoverage, live_until: i64) -> Vec<ConfirmedPaidInterval> {
    let mut intervals = previous.paid_intervals.clone();
    intervals.push(ConfirmedPaidInterval {
        coverage_id: "transfer-term-live".into(),
        source_id: "transferred-source".into(),
        starts_at: live_until - 11 * 24 * 60 * 60,
        paid_until: live_until,
        failed_renewal_id: None,
    });
    intervals
}

#[tokio::test]
async fn enqueue_is_idempotent_and_cancelled_notice_is_not_listed() {
    let _guard = notification_test_lock().lock().await;
    let Some(pool) = pool_or_skip().await else {
        return;
    };
    const USER_ID: &str = "notification-test-idempotency";
    cleanup(&pool, USER_ID).await;
    seed_user(&pool, USER_ID).await;

    let first = notifications::enqueue(&pool, &in_app_intent(USER_ID, "event-1", 1_800_000_000))
        .await
        .expect("enqueue first notice");
    let second = notifications::enqueue(&pool, &in_app_intent(USER_ID, "event-1", 1_900_000_000))
        .await
        .expect("enqueue duplicate notice");
    assert_eq!(first, notifications::EnqueueOutcome::Enqueued);
    assert_eq!(second, notifications::EnqueueOutcome::AlreadyQueued);

    let row = sqlx::query(
        "SELECT extract(epoch FROM due_at)::bigint AS due_at, status, attempt_count \
         FROM cloud_notice_outbox WHERE recipient_user_id=$1 AND event_key='event-1'",
    )
    .bind(USER_ID)
    .fetch_one(&pool)
    .await
    .expect("load idempotent notice");
    assert_eq!(row.get::<i64, _>("due_at"), 1_800_000_000);
    assert_eq!(row.get::<String, _>("status"), "pending");
    assert_eq!(row.get::<i32, _>("attempt_count"), 0);

    assert_eq!(
        notifications::cancel(&pool, USER_ID, "event-1", "renewal-failure-v1")
            .await
            .unwrap(),
        1
    );
    assert!(notifications::list_for_user(&pool, USER_ID)
        .await
        .unwrap()
        .is_empty());
    cleanup(&pool, USER_ID).await;
}

#[tokio::test]
async fn account_notice_list_hides_future_due_rows_but_keeps_due_rows() {
    let _guard = notification_test_lock().lock().await;
    let Some(pool) = pool_or_skip().await else {
        return;
    };
    const USER_ID: &str = "notification-test-due-filter";
    cleanup(&pool, USER_ID).await;
    seed_user(&pool, USER_ID).await;

    let now_epoch: i64 = sqlx::query_scalar("SELECT extract(epoch FROM now())::bigint")
        .fetch_one(&pool)
        .await
        .expect("read database time");
    let due = in_app_intent(USER_ID, "event-due", now_epoch.saturating_sub(1));
    let future = in_app_intent(USER_ID, "event-future", now_epoch.saturating_add(3600));
    notifications::enqueue(&pool, &due)
        .await
        .expect("enqueue due notice");
    notifications::enqueue(&pool, &future)
        .await
        .expect("enqueue future notice");
    // Cancelled rows stay excluded regardless of their due date.
    let cancelled = in_app_intent(USER_ID, "event-cancelled", now_epoch.saturating_sub(1));
    notifications::enqueue(&pool, &cancelled)
        .await
        .expect("enqueue cancellable notice");
    notifications::cancel(&pool, USER_ID, "event-cancelled", "renewal-failure-v1")
        .await
        .expect("cancel due notice");

    let notices = notifications::list_for_user(&pool, USER_ID)
        .await
        .expect("list due notices");
    assert_eq!(notices.len(), 1);
    assert!(notices[0].due_at_epoch <= now_epoch);
    cleanup(&pool, USER_ID).await;
}

struct RetrySender;

#[async_trait]
impl NoticeSender for RetrySender {
    async fn send(&mut self, _delivery: notifications::NoticeDelivery) -> DeliveryOutcome {
        DeliveryOutcome::Retry {
            code: "provider_busy".into(),
        }
    }
}

#[tokio::test]
async fn retry_keeps_lifecycle_due_date_and_records_provider_error() {
    let _guard = notification_test_lock().lock().await;
    let Some(pool) = pool_or_skip().await else {
        return;
    };
    const USER_ID: &str = "notification-test-retry";
    cleanup(&pool, USER_ID).await;
    seed_user(&pool, USER_ID).await;
    notifications::record_verified_email(
        &pool,
        "contact-retry",
        USER_ID,
        "person@example.test",
        1_800_000_000,
    )
    .await
    .expect("record verified contact");
    let intent = NoticeIntent {
        recipient_user_id: USER_ID.into(),
        event_key: "event-retry".into(),
        policy_key: "price-change-v1".into(),
        kind: NoticeKind::PriceChange,
        channel: NoticeChannel::Email,
        contact_id: Some("contact-retry".into()),
        due_at_epoch: 1,
        content: content(),
    };
    notifications::enqueue(&pool, &intent)
        .await
        .expect("enqueue retry notice");
    let mut sender = RetrySender;
    assert!(notifications::run_once(&pool, "worker-retry", &mut sender)
        .await
        .unwrap());

    let row = sqlx::query(
        "SELECT extract(epoch FROM due_at)::bigint AS due_at, status, attempt_count, last_error_code \
         FROM cloud_notice_outbox WHERE recipient_user_id=$1 AND event_key='event-retry'",
    )
    .bind(USER_ID)
    .fetch_one(&pool)
    .await
    .expect("load retried notice");
    assert_eq!(row.get::<i64, _>("due_at"), 1);
    assert_eq!(row.get::<String, _>("status"), "pending");
    assert_eq!(row.get::<i32, _>("attempt_count"), 1);
    assert_eq!(row.get::<String, _>("last_error_code"), "provider_busy");
    cleanup(&pool, USER_ID).await;
}

#[tokio::test]
async fn verified_contact_cannot_be_reassigned_to_another_user() {
    let _guard = notification_test_lock().lock().await;
    let Some(pool) = pool_or_skip().await else {
        return;
    };
    const OWNER: &str = "notification-test-contact-owner";
    const OTHER: &str = "notification-test-contact-other";
    cleanup(&pool, OWNER).await;
    cleanup(&pool, OTHER).await;
    seed_user(&pool, OWNER).await;
    seed_user(&pool, OTHER).await;
    notifications::record_verified_email(
        &pool,
        "contact-owned",
        OWNER,
        "owner@example.test",
        1_800_000_000,
    )
    .await
    .expect("record owner contact");
    assert!(matches!(
        notifications::record_verified_email(
            &pool,
            "contact-owned",
            OTHER,
            "other@example.test",
            1_800_000_001
        )
        .await,
        Err(NoticeError::InvalidKey("contact belongs to another user"))
    ));
    cleanup(&pool, OWNER).await;
    cleanup(&pool, OTHER).await;
}

#[tokio::test]
async fn deleted_contact_fails_queued_email_without_breaking_the_outbox_row() {
    let _guard = notification_test_lock().lock().await;
    let Some(pool) = pool_or_skip().await else {
        return;
    };
    const USER_ID: &str = "notification-test-deleted-contact";
    cleanup(&pool, USER_ID).await;
    seed_user(&pool, USER_ID).await;
    notifications::record_verified_email(
        &pool,
        "contact-deleted",
        USER_ID,
        "person@example.test",
        1_800_000_000,
    )
    .await
    .expect("record verified contact");
    let intent = NoticeIntent {
        recipient_user_id: USER_ID.into(),
        event_key: "event-deleted-contact".into(),
        policy_key: "recovery-v1".into(),
        kind: NoticeKind::RecoveryEnd,
        channel: NoticeChannel::Email,
        contact_id: Some("contact-deleted".into()),
        due_at_epoch: 1,
        content: content(),
    };
    notifications::enqueue(&pool, &intent)
        .await
        .expect("enqueue email notice");
    sqlx::query("DELETE FROM cloud_verified_contacts WHERE contact_id='contact-deleted'")
        .execute(&pool)
        .await
        .expect("delete verified contact");

    let mut sender = RetrySender;
    assert!(
        notifications::run_once(&pool, "worker-deleted-contact", &mut sender)
            .await
            .expect("mark deleted contact unavailable")
    );
    let row = sqlx::query(
        "SELECT status, last_error_code, contact_id FROM cloud_notice_outbox \
         WHERE recipient_user_id=$1 AND event_key='event-deleted-contact'",
    )
    .bind(USER_ID)
    .fetch_one(&pool)
    .await
    .expect("load deleted-contact notice");
    assert_eq!(row.get::<String, _>("status"), "failed");
    assert_eq!(row.get::<String, _>("last_error_code"), "contact_missing");
    assert_eq!(row.get::<Option<String>, _>("contact_id"), None);
    cleanup(&pool, USER_ID).await;
}
