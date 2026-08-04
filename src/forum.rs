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
    pub(crate) replies: Vec<Reply>,
}

pub(crate) struct Reply {
    pub(crate) body: String,
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
    title: String,
    body: String,
}

#[derive(sqlx::FromRow)]
struct ThreadPageRow {
    board_slug: String,
    board_name: String,
    board_description: String,
    thread_id: u64,
    thread_title: String,
    thread_body: String,
}

#[derive(sqlx::FromRow)]
struct ReplyRow {
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
        SELECT t.id, t.title, t.body
        FROM threads t
        JOIN boards b ON b.id = t.board_id
        WHERE b.slug = ?
        AND b.status = 'approved'
        AND t.status = 'visible'
        ORDER BY t.created_at, t.id
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
            t.body AS thread_body
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
        SELECT body 
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
    .map(|reply| Reply { body: reply.body })
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
            title: thread_row.thread_title,
            body: thread_row.thread_body,
            replies,
        },
    )))
}
