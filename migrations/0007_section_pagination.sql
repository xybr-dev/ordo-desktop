-- Per-section pagination state (T2.8). Approval moves from the Arrangement to
-- each of its sections, so readiness can name the sections that need review.
-- Every section inherits its Arrangement's state (an Approved Arrangement keeps
-- all its sections approved; a Needs Review one marks all of them for review),
-- then the Arrangement column is dropped.

ALTER TABLE arrangement_section ADD COLUMN pagination_state TEXT NOT NULL DEFAULT 'draft' CHECK (pagination_state IN ('draft', 'approved', 'needs_review'));

UPDATE arrangement_section SET pagination_state = COALESCE(
    (SELECT arrangement.pagination_state FROM arrangement WHERE arrangement.id = arrangement_section.arrangement_id),
    'draft'
);

ALTER TABLE arrangement DROP COLUMN pagination_state;
