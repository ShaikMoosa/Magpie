//! Category CRUD with nesting, colours and safe deletion.

use rusqlite::{Connection, OptionalExtension, Row, params};

use crate::bookmarks;
use crate::db::{new_id, now};
use crate::error::{Error, Result};
use crate::models::{Category, DeleteCategory, NewCategory, UpdateCategory};

const SELECT: &str = "SELECT c.id, c.name, c.description, c.color, c.icon, c.parent_id, c.sort_order,
    c.created_at, c.updated_at,
    (SELECT COUNT(*) FROM bookmarks b WHERE b.category_id = c.id AND b.deleted_at IS NULL)
    FROM categories c";

fn from_row(r: &Row) -> rusqlite::Result<Category> {
    Ok(Category {
        id: r.get(0)?,
        name: r.get(1)?,
        description: r.get(2)?,
        color: r.get(3)?,
        icon: r.get(4)?,
        parent_id: r.get(5)?,
        sort_order: r.get(6)?,
        created_at: r.get(7)?,
        updated_at: r.get(8)?,
        bookmark_count: r.get(9)?,
    })
}

pub fn list(conn: &Connection) -> Result<Vec<Category>> {
    let mut stmt = conn.prepare(&format!("{SELECT} ORDER BY c.sort_order, c.name COLLATE NOCASE"))?;
    let rows = stmt.query_map([], from_row)?;
    Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
}

pub fn get(conn: &Connection, id: &str) -> Result<Category> {
    conn.query_row(&format!("{SELECT} WHERE c.id = ?1"), [id], from_row)
        .optional()?
        .ok_or(Error::NotFound("Category"))
}

/// Validate `#RRGGBB`.
pub fn is_hex_colour(s: &str) -> bool {
    s.len() == 7 && s.starts_with('#') && s[1..].chars().all(|c| c.is_ascii_hexdigit())
}

fn validate(name: &str, colour: &str) -> Result<String> {
    let name = name.trim();
    if name.is_empty() {
        return Err(Error::Validation("Give the category a name.".into()));
    }
    if name.chars().count() > 40 {
        return Err(Error::Validation("Category names can be up to 40 characters.".into()));
    }
    if !is_hex_colour(colour) {
        return Err(Error::Validation("Colour must be a hex value like #0D9488.".into()));
    }
    Ok(name.to_string())
}

fn clean(s: Option<String>) -> Option<String> {
    s.map(|s| s.trim().to_string()).filter(|s| !s.is_empty())
}

fn map_unique(e: rusqlite::Error) -> Error {
    if let rusqlite::Error::SqliteFailure(f, _) = &e
        && f.code == rusqlite::ErrorCode::ConstraintViolation
    {
        return Error::Validation("A category with this name already exists here.".into());
    }
    Error::Db(e)
}

/// All descendant ids of `id` (not including `id`).
fn descendants(conn: &Connection, id: &str) -> Result<Vec<String>> {
    let mut stmt = conn.prepare(
        "WITH RECURSIVE sub(id) AS (SELECT id FROM categories WHERE parent_id = ?1
            UNION ALL SELECT c.id FROM categories c JOIN sub ON c.parent_id = sub.id)
         SELECT id FROM sub",
    )?;
    let ids = stmt.query_map([id], |r| r.get(0))?.collect::<rusqlite::Result<Vec<String>>>()?;
    Ok(ids)
}

fn check_parent(conn: &Connection, id: Option<&str>, parent: &Option<String>) -> Result<()> {
    let Some(parent) = parent else { return Ok(()) };
    get(conn, parent)?;
    if let Some(id) = id
        && (parent == id || descendants(conn, id)?.iter().any(|d| d == parent))
    {
        return Err(Error::Validation("A category can't be placed inside itself.".into()));
    }
    Ok(())
}

pub fn create(conn: &Connection, input: NewCategory) -> Result<Category> {
    let name = validate(&input.name, &input.color)?;
    check_parent(conn, None, &input.parent_id)?;
    let id = new_id();
    let ts = now();
    let order: i64 = conn.query_row("SELECT COALESCE(MAX(sort_order), -1) + 1 FROM categories", [], |r| r.get(0))?;
    conn.execute(
        "INSERT INTO categories (id, name, description, color, icon, parent_id, sort_order, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?8)",
        params![id, name, clean(input.description), input.color.to_uppercase(), clean(input.icon), input.parent_id, order, ts],
    )
    .map_err(map_unique)?;
    get(conn, &id)
}

pub fn update(conn: &Connection, input: UpdateCategory) -> Result<Category> {
    let current = get(conn, &input.id)?;
    let name = validate(&input.name, &input.color)?;
    check_parent(conn, Some(&input.id), &input.parent_id)?;
    conn.execute(
        "UPDATE categories SET name = ?2, description = ?3, color = ?4, icon = ?5, parent_id = ?6, updated_at = ?7
         WHERE id = ?1",
        params![input.id, name, clean(input.description), input.color.to_uppercase(), clean(input.icon), input.parent_id, now()],
    )
    .map_err(map_unique)?;
    if current.name != name {
        bookmarks::reindex_category(conn, &input.id)?;
    }
    get(conn, &input.id)
}

/// Delete a category without deleting its bookmarks.
pub fn delete(conn: &Connection, input: DeleteCategory) -> Result<()> {
    get(conn, &input.id)?;
    let subs = descendants(conn, &input.id)?;
    if let Some(target) = &input.move_to {
        get(conn, target)?;
        if target == &input.id || (!input.keep_subcategories && subs.contains(target)) {
            return Err(Error::Validation("Choose a category that isn't being deleted.".into()));
        }
    }

    // Categories whose bookmarks must move: this one, plus all descendants
    // when the subcategories are deleted too.
    let mut emptied = vec![input.id.clone()];
    if !input.keep_subcategories {
        emptied.extend(subs.iter().cloned());
    }

    let tx = conn.unchecked_transaction()?;
    let mut moved: Vec<String> = Vec::new();
    for cat in &emptied {
        let mut stmt = tx.prepare("SELECT id FROM bookmarks WHERE category_id = ?1")?;
        moved.extend(stmt.query_map([cat], |r| r.get::<_, String>(0))?.collect::<rusqlite::Result<Vec<_>>>()?);
        tx.execute(
            "UPDATE bookmarks SET category_id = ?2, updated_at = ?3 WHERE category_id = ?1",
            params![cat, input.move_to, now()],
        )?;
    }
    if input.keep_subcategories {
        // Direct children become top-level; deeper levels keep their parent.
        tx.execute("UPDATE categories SET parent_id = NULL WHERE parent_id = ?1", [&input.id])?;
    } else {
        for sub in subs.iter().rev() {
            tx.execute("DELETE FROM categories WHERE id = ?1", [sub])?;
        }
    }
    tx.execute("DELETE FROM categories WHERE id = ?1", [&input.id])?;
    for id in &moved {
        bookmarks::reindex(&tx, id)?;
    }
    tx.commit()?;
    Ok(())
}

/// Persist a new display order (ids in the desired order).
pub fn reorder(conn: &Connection, ids: &[String]) -> Result<()> {
    let tx = conn.unchecked_transaction()?;
    for (i, id) in ids.iter().enumerate() {
        tx.execute("UPDATE categories SET sort_order = ?2 WHERE id = ?1", params![id, i as i64])?;
    }
    tx.commit()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::open_in_memory;
    use crate::models::{BookmarkQuery, NewBookmark, View};

    fn cat(conn: &Connection, name: &str, parent: Option<&str>) -> Category {
        create(
            conn,
            NewCategory {
                name: name.into(),
                description: None,
                color: "#2563eb".into(),
                icon: None,
                parent_id: parent.map(String::from),
            },
        )
        .unwrap()
    }

    fn bm(conn: &Connection, url: &str, cat: &str) -> String {
        bookmarks::create(conn, NewBookmark { url: url.into(), category_id: Some(cat.into()), ..Default::default() })
            .unwrap()
            .id
    }

    #[test]
    fn validation() {
        let c = open_in_memory().unwrap();
        let bad = |name: &str, colour: &str| {
            create(&c, NewCategory { name: name.into(), description: None, color: colour.into(), icon: None, parent_id: None })
                .is_err()
        };
        assert!(bad("", "#000000"));
        assert!(bad("Ok", "red"));
        assert!(bad("Ok", "#12345"));
        let d = cat(&c, "Design", None);
        assert_eq!(d.color, "#2563EB");
        assert!(bad("design", "#000000"), "sibling names are unique, case-insensitive");
        cat(&c, "Design", Some(&d.id)); // same name under a different parent is fine
    }

    #[test]
    fn no_cycles() {
        let c = open_in_memory().unwrap();
        let a = cat(&c, "A", None);
        let b = cat(&c, "B", Some(&a.id));
        let err = update(
            &c,
            UpdateCategory { id: a.id.clone(), name: "A".into(), description: None, color: "#000000".into(), icon: None, parent_id: Some(b.id) },
        );
        assert!(err.is_err());
    }

    #[test]
    fn category_view_includes_subcategories() {
        let c = open_in_memory().unwrap();
        let d = cat(&c, "Design", None);
        let k = cat(&c, "UI Kits", Some(&d.id));
        bm(&c, "https://a.com", &d.id);
        bm(&c, "https://b.com", &k.id);
        let q = BookmarkQuery { view: View::Category, category_id: Some(d.id.clone()), ..Default::default() };
        assert_eq!(bookmarks::list(&c, &q).unwrap().len(), 2);
    }

    #[test]
    fn delete_moves_bookmarks_never_deletes_them() {
        let c = open_in_memory().unwrap();
        let d = cat(&c, "Design", None);
        let k = cat(&c, "UI Kits", Some(&d.id));
        let r = cat(&c, "Research", None);
        let a = bm(&c, "https://a.com", &d.id);
        let b = bm(&c, "https://b.com", &k.id);

        // Keep subcategories: UI Kits becomes top-level, Design's bookmark → Research.
        delete(&c, DeleteCategory { id: d.id.clone(), move_to: Some(r.id.clone()), keep_subcategories: true }).unwrap();
        assert_eq!(bookmarks::get(&c, &a).unwrap().category_id, Some(r.id.clone()));
        assert_eq!(get(&c, &k.id).unwrap().parent_id, None);
        assert_eq!(bookmarks::get(&c, &b).unwrap().category_id, Some(k.id.clone()));

        // Delete with subcategories → Uncategorized.
        let x = cat(&c, "X", None);
        let y = cat(&c, "Y", Some(&x.id));
        let yb = bm(&c, "https://y.com", &y.id);
        delete(&c, DeleteCategory { id: x.id.clone(), move_to: None, keep_subcategories: false }).unwrap();
        assert!(get(&c, &y.id).is_err());
        assert_eq!(bookmarks::get(&c, &yb).unwrap().category_id, None);
        assert_eq!(bookmarks::counts(&c).unwrap().all, 3);
    }
}
