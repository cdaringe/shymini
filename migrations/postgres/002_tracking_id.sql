-- Add short tracking_id for use in tracker URLs (avoids exposing UUID)
ALTER TABLE services ADD COLUMN IF NOT EXISTS tracking_id TEXT;

-- Create unique index
CREATE UNIQUE INDEX IF NOT EXISTS idx_services_tracking_id ON services(tracking_id);
