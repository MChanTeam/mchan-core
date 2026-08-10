pub(crate) struct Board {
    pub(crate) slug: String,
    pub(crate) name: String,
    pub(crate) description: String,
    pub(crate) threads: Vec<Thread>,
}

pub(crate) struct Thread {
    pub(crate) id: u64,
    pub(crate) title: String,
    pub(crate) body: String,
    pub(crate) poster_id: String,
    pub(crate) replies: Vec<Reply>,
}

pub(crate) struct Reply {
    pub(crate) id: u64,
    pub(crate) body: String,
    pub(crate) poster_id: String,
}

#[derive(sqlx::FromRow)]
struct BoardRow {
    slug: String,
    name: String,
    description: String,
}

#[derive(sqlx::FromRow)]
struct ThreadRow {
    id: u64,
    poster_id: String,
    title: String,
    body: String,
}

#[derive(sqlx::FromRow)]
struct ThreadPageRow {
    board_slug: String,
    board_name: String,
    board_description: String,
    thread_id: u64,
    poster_id: String,
    thread_title: String,
    thread_body: String,
}

#[derive(sqlx::FromRow)]
struct ReplyRow {
    id: u64,
    poster_id: String,
    body: String,
}

pub(crate) async fn load_approved_boards(
    pool: &sqlx::SqlitePool,
) -> Result<Vec<Board>, sqlx::Error> {
    let rows = sqlx::query_as::<_, BoardRow>(
        r#"
        SELECT slug, name, description
        FROM boards 
        WHERE status = 'approved' 
        ORDER BY name
        "#,
    )
    .fetch_all(pool)
    .await?;

    Ok(rows
        .into_iter()
        .map(|row| Board {
            slug: row.slug,
            name: row.name,
            description: row.description,
            threads: Vec::new(),
        })
        .collect())
}

pub(crate) async fn load_board(
    pool: &sqlx::SqlitePool,
    slug: &str,
) -> Result<Option<Board>, sqlx::Error> {
    let Some(board_row) = sqlx::query_as::<_, BoardRow>(
        r#"
        SELECT slug, name, description
        FROM boards
        WHERE slug = ? AND status = 'approved'
        "#,
    )
    .bind(slug)
    .fetch_optional(pool)
    .await?
    else {
        return Ok(None);
    };

    let threads = sqlx::query_as::<_, ThreadRow>(
        r#"
        SELECT t.id, t.title, t.body, t.poster_id
        FROM threads t
        JOIN boards b ON b.id = t.board_id
        WHERE b.slug = ?
        AND b.status = 'approved'
        AND t.status = 'visible'
        ORDER BY t.created_at DESC, t.id DESC
        "#,
    )
    .bind(slug)
    .fetch_all(pool)
    .await?
    .into_iter()
    .map(|thread| Thread {
        id: thread.id,
        title: thread.title,
        body: thread.body,
        poster_id: thread.poster_id,
        replies: Vec::new(),
    })
    .collect();

    Ok(Some(Board {
        slug: board_row.slug,
        name: board_row.name,
        description: board_row.description,
        threads,
    }))
}

pub(crate) async fn create_thread(
    pool: &sqlx::SqlitePool,
    board_slug: &str,
    title: &str,
    body: &str,
    anonymous_token: &str,
) -> Result<Option<u64>, sqlx::Error> {
    let mut transaction = pool.begin().await?;
    let Some(board_id) = sqlx::query_scalar::<_, i64>(
        r#"
        SELECT id
        FROM boards
        WHERE slug = ? AND status = 'approved'
        "#,
    )
    .bind(board_slug)
    .fetch_optional(&mut *transaction)
    .await?
    else {
        return Ok(None);
    };

    let result = sqlx::query(
        r#"
        INSERT INTO threads (board_id, title, body, status)
        VALUES (?, ?, ?, 'visible')
        "#,
    )
    .bind(board_id)
    .bind(title)
    .bind(body)
    .execute(&mut *transaction)
    .await?;

    let thread_id = result.last_insert_rowid() as u64;
    let poster_id = crate::thread_poster_id(anonymous_token, thread_id);

    sqlx::query(
        r#"
        UPDATE threads
        SET poster_id = ?
        WHERE id = ?
        "#,
    )
    .bind(&poster_id)
    .bind(thread_id as i64)
    .execute(&mut *transaction)
    .await?;

    transaction.commit().await?;

    Ok(Some(thread_id))
}

pub(crate) async fn create_reply(
    pool: &sqlx::SqlitePool,
    thread_id: u64,
    body: &str,
    poster_id: &str,
) -> Result<bool, sqlx::Error> {
    let Some(_) = sqlx::query_scalar::<_, i64>(
        r#"
        SELECT id 
        FROM threads 
        WHERE id = ? AND status = 'visible'
        "#,
    )
    .bind(thread_id as i64)
    .fetch_optional(pool)
    .await?
    else {
        return Ok(false);
    };

    sqlx::query(
        r#"
        INSERT INTO replies (thread_id, body, poster_id, status)
        VALUES (?, ?, ?, 'visible')
        "#,
    )
    .bind(thread_id as i64)
    .bind(body)
    .bind(poster_id)
    .execute(pool)
    .await?;

    Ok(true)
}

pub(crate) async fn report_thread(
    pool: &sqlx::SqlitePool,
    thread_id: u64,
    reason: &str,
) -> Result<bool, sqlx::Error> {
    let Some(_) = sqlx::query_scalar::<_, i64>(
        r#"
        SELECT id
        FROM threads
        WHERE id = ?
          AND status IN ('visible', 'locked')
        "#,
    )
    .bind(thread_id as i64)
    .fetch_optional(pool)
    .await?
    else {
        return Ok(false);
    };

    sqlx::query(
        r#"
        INSERT INTO reports (thread_id, reason, status)
        VALUES (?, ?, 'pending')
        "#,
    )
    .bind(thread_id as i64)
    .bind(reason)
    .execute(pool)
    .await?;

    Ok(true)
}

pub(crate) async fn report_reply(
    pool: &sqlx::SqlitePool,
    reply_id: u64,
    reason: &str,
) -> Result<Option<u64>, sqlx::Error> {
    let Some(thread_id) = sqlx::query_scalar::<_, i64>(
        r#"
        SELECT thread_id
        FROM replies
        WHERE id = ? AND status = 'visible'
        "#,
    )
    .bind(reply_id as i64)
    .fetch_optional(pool)
    .await?
    else {
        return Ok(None);
    };

    sqlx::query(
        r#"
        INSERT INTO reports (reply_id, reason, status)
        VALUES (?, ?, 'pending')
        "#,
    )
    .bind(reply_id as i64)
    .bind(reason)
    .execute(pool)
    .await?;

    Ok(Some(thread_id as u64))
}

pub(crate) async fn load_thread(
    pool: &sqlx::SqlitePool,
    id: u64,
) -> Result<Option<(Board, Thread)>, sqlx::Error> {
    let Some(thread_row) = sqlx::query_as::<_, ThreadPageRow>(
        r#"
        SELECT
            b.slug AS board_slug,
            b.name AS board_name,
            b.description AS board_description,
            t.id AS thread_id,
            t.title AS thread_title,
            t.body AS thread_body,
            t.poster_id AS poster_id
        FROM threads t 
        JOIN boards b ON b.id = t.board_id
        WHERE t.id = ?
            AND t.status IN ('visible', "locked")
            AND b.status = 'approved'
        "#,
    )
    .bind(id as i64)
    .fetch_optional(pool)
    .await?
    else {
        return Ok(None);
    };

    let replies = sqlx::query_as::<_, ReplyRow>(
        r#"
        SELECT id, body, poster_id
        FROM replies 
        WHERE thread_id = ?
            AND status = 'visible'
        ORDER BY created_at, id
        "#,
    )
    .bind(id as i64)
    .fetch_all(pool)
    .await?
    .into_iter()
    .map(|reply| Reply {
        id: reply.id,
        body: reply.body,
        poster_id: reply.poster_id,
    })
    .collect();

    Ok(Some((
        Board {
            slug: thread_row.board_slug,
            name: thread_row.board_name,
            description: thread_row.board_description,
            threads: Vec::new(),
        },
        Thread {
            id: thread_row.thread_id,
            poster_id: thread_row.poster_id,
            title: thread_row.thread_title,
            body: thread_row.thread_body,
            replies,
        },
    )))
}
