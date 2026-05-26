#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct User {
    pub id: String,
    pub nickname: String,
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct Team {
    pub id: String,
    pub name: String,
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct Member {
    pub id: String,

    pub user_id: String,
    pub user_nickname: String,

    pub team_id: String,
    pub team: Option<Team>,

    pub role_mask: i32,

    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct Workset {
    pub id: String,

    pub name: String,

    pub team_id: String,
    pub index: i32,

    pub comic_count: i32,

    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct Comic {
    pub id: String,

    pub workset_id: String,

    pub index: i32,

    pub title: String,
    pub author: String,

    pub chapter_count: i32,

    pub last_active_at: i64,

    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct Chapter {
    pub id: String,

    pub comic_id: String,

    pub index: i32,

    pub subtitle: String,

    pub page_count: i32,
    pub total_unit_count: i32,
    pub translated_unit_count: i32,
    pub proofread_unit_count: i32,

    pub uploaded_at: Option<i64>,
    pub translating_at: Option<i64>,
    pub translated_at: Option<i64>,
    pub proofreading_at: Option<i64>,
    pub proofread_at: Option<i64>,
    pub typesetting_at: Option<i64>,
    pub typeset_at: Option<i64>,
    pub reviewed_at: Option<i64>,
    pub published_at: Option<i64>,

    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct Assignment {
    pub id: String,

    pub chapter_id: String,
    pub chapter: Option<Chapter>,

    pub user_id: String,
    pub user: Option<User>,

    pub assigned_raw_provider_at: Option<i64>,
    pub assigned_translator_at: Option<i64>,
    pub assigned_proofreader_at: Option<i64>,
    pub assigned_typesetter_at: Option<i64>,
    pub assigned_reviewer_at: Option<i64>,
    pub assigned_publisher_at: Option<i64>,

    pub created_at: i64,
    pub updated_at: i64,
}
