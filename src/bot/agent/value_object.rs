/// Basic user info returned from Poprako-S.
///
/// Corresponds to `val.UserVal`.
#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct User {
    /// Unique user identifier.
    pub id: String,

    /// Display nickname.
    pub nickname: String,

    /// QQ identifier.
    #[serde(default)]
    pub qid: String,
}

/// Team info returned from Poprako-S.
///
/// Corresponds to `val.TeamVal`.
#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct Team {
    /// Unique team identifier.
    pub id: String,

    /// Display name.
    pub name: String,
}

/// Team membership record.
///
/// Corresponds to `val.MemberVal`.
///
/// ## Role mask bits (`role_mask`)
///
/// | Bit | Value | Role | Scope |
/// |-----|-------|------|-------|
/// | 0   | 0x001 | Raw provider | Chapter assignment |
/// | 1   | 0x002 | Translator | Chapter assignment |
/// | 2   | 0x004 | Proofreader | Chapter assignment |
/// | 3   | 0x008 | Typesetter | Chapter assignment |
/// | 4   | 0x010 | Redrawer | Chapter assignment |
/// | 5   | 0x020 | Reviewer | Chapter assignment |
/// | 6   | 0x040 | Publisher | Chapter assignment |
/// | 7   | 0x080 | Admin | Team level only |
#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct Member {
    /// Unique membership record identifier.
    pub id: String,

    /// User identifier of the member.
    pub user_id: String,

    /// Display nickname of the member (denormalized).
    pub user_nickname: String,

    /// Team identifier.
    pub team_id: String,

    /// Resolved team object, present only when `includes=team` is requested.
    #[serde(default)]
    pub team: Option<Team>,

    /// Resolved user object, present only when `includes=user` is requested.
    #[serde(default)]
    pub user: Option<User>,

    /// Bitmask of assigned roles. See struct-level doc for bit definitions.
    pub role_mask: u32,

    /// Creation timestamp in milliseconds since Unix epoch.
    pub created_at: i64,

    /// Last update timestamp in milliseconds since Unix epoch.
    pub updated_at: i64,
}

/// Workset (a collection of comics) under a team.
///
/// Corresponds to `val.WorksetVal`.
#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct Workset {
    /// Unique workset identifier.
    pub id: String,

    /// Display name.
    pub name: String,

    /// Optional description.
    #[serde(default)]
    pub description: Option<String>,

    /// Resolved team object, present only when relation include requests it.
    #[serde(default)]
    pub team: Option<Team>,

    /// Owning team identifier.
    pub team_id: String,

    /// Display ordering index within the team.
    pub index: i32,

    /// Number of active comics in this workset.
    pub comic_count: i32,

    /// Creation timestamp in milliseconds since Unix epoch.
    pub created_at: i64,

    /// Last update timestamp in milliseconds since Unix epoch.
    pub updated_at: i64,
}

/// Comic (a series) under a workset.
///
/// Corresponds to `val.ComicVal`.
#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct Comic {
    /// Unique comic identifier.
    pub id: String,

    /// Owning workset identifier.
    pub workset_id: String,

    /// Display ordering index within the workset.
    pub index: i32,

    /// Comic title.
    pub title: String,

    /// Original author name.
    pub author: String,

    /// Optional description.
    #[serde(default)]
    pub description: Option<String>,

    /// Whether the comic is marked as completed.
    #[serde(default)]
    pub is_completed: bool,

    /// Whether the cover image has been uploaded.
    #[serde(default)]
    pub cover_uploaded: bool,

    /// Cover image URL.
    #[serde(default)]
    pub cover_url: String,

    /// Number of chapters in this comic.
    pub chapter_count: i32,

    /// Creator user id.
    #[serde(default)]
    pub creator_id: String,

    /// Resolved creator user object.
    #[serde(default)]
    pub creator: Option<User>,

    /// Resolved workset object.
    #[serde(default)]
    pub workset: Option<Workset>,

    /// Last activity timestamp in milliseconds since Unix epoch
    /// (e.g. the latest chapter creation or workflow action).
    pub last_active_at: i64,

    /// Creation timestamp in milliseconds since Unix epoch.
    pub created_at: i64,

    /// Last update timestamp in milliseconds since Unix epoch.
    pub updated_at: i64,
}

/// Chapter under a comic, with workflow lifecycle timestamps.
///
/// Corresponds to `val.ChapterVal`.
///
/// ## Workflow lifecycle
///
/// Each production phase has one or two timestamp fields.
/// A `{phase}_at` field being `None` means that phase has not reached its terminal state.
/// A `{phase}ing_at` field being `Some` means the phase is currently ongoing.
///
/// | Phase | Start timestamp | Complete timestamp | Triggered by |
/// |-------|----------------|--------------------|--------------|
/// | Upload | (not tracked) | `uploaded_at` | `upload_complete` |
/// | Translate | `translating_at` | `translated_at` | `translate_start` / `translate_complete` |
/// | Proofread | `proofreading_at` | `proofread_at` | `proofread_start` / `proofread_complete` |
/// | Typeset | `typesetting_at` | `typeset_at` | `typeset_start` / `typeset_complete` |
/// | Review | (not tracked) | `reviewed_at` | `review_complete` |
/// | Publish | (not tracked) | `published_at` | `publish_complete` |
#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct Chapter {
    /// Unique chapter identifier.
    pub id: String,

    /// Owning comic identifier.
    pub comic_id: String,

    /// Resolved comic object.
    #[serde(default)]
    pub comic: Option<Comic>,

    /// Whether this chapter is pinned.
    #[serde(default)]
    pub is_pinned: bool,

    /// Display ordering index within the comic.
    pub index: i32,

    /// Chapter subtitle (e.g. "CH.1", "Vol.2 Ep.3").
    pub subtitle: String,

    /// Number of pages in this chapter.
    pub page_count: i32,

    /// Total number of translation units across all pages.
    pub total_unit_count: i32,

    /// Number of units that have been translated.
    pub translated_unit_count: i32,

    /// Number of units that have been proofread.
    pub proofread_unit_count: i32,

    /// Creator user id.
    #[serde(default)]
    pub creator_id: String,

    /// Resolved creator user object.
    #[serde(default)]
    pub creator: Option<User>,

    /// Timestamp when all pages were uploaded, in milliseconds since Unix epoch.
    #[serde(default)]
    pub uploaded_at: Option<i64>,

    /// Timestamp when translation started, in milliseconds since Unix epoch.
    #[serde(default)]
    pub translating_at: Option<i64>,

    /// Timestamp when translation completed, in milliseconds since Unix epoch.
    #[serde(default)]
    pub translated_at: Option<i64>,

    /// Timestamp when proofreading started, in milliseconds since Unix epoch.
    #[serde(default)]
    pub proofreading_at: Option<i64>,

    /// Timestamp when proofreading completed, in milliseconds since Unix epoch.
    #[serde(default)]
    pub proofread_at: Option<i64>,

    /// Timestamp when typesetting started, in milliseconds since Unix epoch.
    #[serde(default)]
    pub typesetting_at: Option<i64>,

    /// Timestamp when typesetting completed, in milliseconds since Unix epoch.
    #[serde(default)]
    pub typeset_at: Option<i64>,

    /// Timestamp when review completed, in milliseconds since Unix epoch.
    #[serde(default)]
    pub reviewed_at: Option<i64>,

    /// Timestamp when publish completed, in milliseconds since Unix epoch.
    #[serde(default)]
    pub published_at: Option<i64>,

    /// Creation timestamp in milliseconds since Unix epoch.
    pub created_at: i64,

    /// Last update timestamp in milliseconds since Unix epoch.
    pub updated_at: i64,
}

/// Assignment of one user to a chapter, with per-role assignment timestamps.
///
/// Corresponds to `val.AssignmentVal`.
///
/// The set of assigned roles is encoded entirely by which `assigned_{role}_at`
/// fields are `Some` — no separate `role_mask` is needed.
/// See [`Member`] for role bit definitions.
///
/// ## Role timestamps
///
/// | Field | Meaning |
/// |-------|---------|
/// | `assigned_raw_provider_at` | User was given the raw-provider role at this time |
/// | `assigned_translator_at` | User was given the translator role at this time |
/// | `assigned_proofreader_at` | User was given the proofreader role at this time |
/// | `assigned_typesetter_at` | User was given the typesetter role at this time |
/// | `assigned_redrawer_at` | User was given the redrawer role at this time |
/// | `assigned_reviewer_at` | User was given the reviewer role at this time |
/// | `assigned_publisher_at` | User was given the publisher role at this time |
///
/// `None` means the user has never held that role in this assignment.
#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct Assignment {
    /// Unique assignment identifier.
    pub id: String,

    /// Chapter this assignment belongs to.
    pub chapter_id: String,

    /// Resolved chapter object, present only when `includes=chapter` is requested.
    #[serde(default)]
    pub chapter: Option<Chapter>,

    /// User identifier of the assignee.
    pub user_id: String,

    /// Resolved user object, present only when `includes=user` is requested.
    #[serde(default)]
    pub user: Option<User>,

    /// Bitmask of assigned roles.
    #[serde(default)]
    pub role_mask: u32,

    /// Timestamp when raw-provider role was assigned, in milliseconds since Unix epoch.
    #[serde(default)]
    pub assigned_raw_provider_at: Option<i64>,

    /// Timestamp when translator role was assigned, in milliseconds since Unix epoch.
    #[serde(default)]
    pub assigned_translator_at: Option<i64>,

    /// Timestamp when proofreader role was assigned, in milliseconds since Unix epoch.
    #[serde(default)]
    pub assigned_proofreader_at: Option<i64>,

    /// Timestamp when typesetter role was assigned, in milliseconds since Unix epoch.
    #[serde(default)]
    pub assigned_typesetter_at: Option<i64>,

    /// Timestamp when redrawer role was assigned, in milliseconds since Unix epoch.
    #[serde(default)]
    pub assigned_redrawer_at: Option<i64>,

    /// Timestamp when reviewer role was assigned, in milliseconds since Unix epoch.
    #[serde(default)]
    pub assigned_reviewer_at: Option<i64>,

    /// Timestamp when publisher role was assigned, in milliseconds since Unix epoch.
    #[serde(default)]
    pub assigned_publisher_at: Option<i64>,

    /// Creation timestamp in milliseconds since Unix epoch.
    pub created_at: i64,

    /// Last update timestamp in milliseconds since Unix epoch.
    pub updated_at: i64,
}
