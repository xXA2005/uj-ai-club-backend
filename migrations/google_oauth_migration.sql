-- Migration to add Google OAuth support and university/major fields
-- Remove password_hash column since we're moving to Google OAuth only
-- Add google_id, university, and major fields

-- Add Google ID and profile fields first
ALTER TABLE users ADD COLUMN google_id VARCHAR(255) UNIQUE;
ALTER TABLE users ADD COLUMN university VARCHAR(255);
ALTER TABLE users ADD COLUMN major VARCHAR(255);
ALTER TABLE users ADD COLUMN university_major_set BOOLEAN NOT NULL DEFAULT FALSE;

-- Make password_hash nullable first (for gradual migration)
ALTER TABLE users ALTER COLUMN password_hash DROP NOT NULL;

-- Make phone_num not required (was already nullable)
-- Add index for Google ID
CREATE INDEX idx_users_google_id ON users(google_id);

-- Update existing users to have a flag indicating they need to set university/major
UPDATE users SET university_major_set = FALSE WHERE university IS NULL OR major IS NULL;

-- After migration is complete and all users are using Google OAuth,
-- we can remove the password_hash column entirely:
-- ALTER TABLE users DROP COLUMN password_hash;
-- ALTER TABLE users DROP COLUMN phone_num; -- if not needed

-- For now, we keep password_hash as nullable to allow for gradual migration
-- You can run this later to clean up:
-- ALTER TABLE users DROP COLUMN password_hash;
-- ALTER TABLE users DROP COLUMN phone_num;