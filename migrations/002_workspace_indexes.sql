ALTER TABLE story_entities ADD COLUMN project_id TEXT REFERENCES projects(id) ON DELETE CASCADE;

CREATE INDEX IF NOT EXISTS idx_books_project_volume ON books(project_id, volume);
CREATE INDEX IF NOT EXISTS idx_chapters_book_order ON chapters(book_id, order_index);
CREATE INDEX IF NOT EXISTS idx_story_entities_project ON story_entities(project_id, updated_at);
