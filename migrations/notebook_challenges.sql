-- Notebook Challenges Migration
-- Adds support for Jupyter notebook challenges with auto-grading

-- Add notebook-related columns to challenges table
ALTER TABLE challenges
ADD COLUMN IF NOT EXISTS notebook_url TEXT,
ADD COLUMN IF NOT EXISTS max_score INTEGER DEFAULT 100,
ADD COLUMN IF NOT EXISTS grading_criteria JSONB,
ADD COLUMN IF NOT EXISTS challenge_type VARCHAR(50) DEFAULT 'general' CHECK (challenge_type IN ('general', 'notebook'));

-- Update existing challenges to have challenge_type
UPDATE challenges SET challenge_type = 'general' WHERE challenge_type IS NULL;

-- Create challenge_submissions table
CREATE TABLE IF NOT EXISTS challenge_submissions (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    challenge_id INTEGER NOT NULL REFERENCES challenges(id) ON DELETE CASCADE,
    notebook_content JSONB NOT NULL,
    score INTEGER,
    max_score INTEGER NOT NULL,
    feedback TEXT,
    status VARCHAR(50) DEFAULT 'pending' CHECK (status IN ('pending', 'grading', 'graded', 'error')),
    execution_time_ms INTEGER,
    submitted_at TIMESTAMP NOT NULL DEFAULT NOW(),
    graded_at TIMESTAMP,
    created_at TIMESTAMP NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMP NOT NULL DEFAULT NOW()
);

-- Create indexes for challenge_submissions
CREATE INDEX IF NOT EXISTS idx_challenge_submissions_user_id ON challenge_submissions(user_id);
CREATE INDEX IF NOT EXISTS idx_challenge_submissions_challenge_id ON challenge_submissions(challenge_id);
CREATE INDEX IF NOT EXISTS idx_challenge_submissions_status ON challenge_submissions(status);
CREATE INDEX IF NOT EXISTS idx_challenge_submissions_submitted_at ON challenge_submissions(submitted_at DESC);

-- Create challenge_attempts table for tracking user progress
CREATE TABLE IF NOT EXISTS challenge_attempts (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    challenge_id INTEGER NOT NULL REFERENCES challenges(id) ON DELETE CASCADE,
    attempt_number INTEGER NOT NULL,
    best_score INTEGER DEFAULT 0,
    total_attempts INTEGER DEFAULT 0,
    last_attempt_at TIMESTAMP,
    created_at TIMESTAMP NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMP NOT NULL DEFAULT NOW(),
    UNIQUE(user_id, challenge_id)
);

-- Create indexes for challenge_attempts
CREATE INDEX IF NOT EXISTS idx_challenge_attempts_user_id ON challenge_attempts(user_id);
CREATE INDEX IF NOT EXISTS idx_challenge_attempts_challenge_id ON challenge_attempts(challenge_id);
CREATE INDEX IF NOT EXISTS idx_challenge_attempts_best_score ON challenge_attempts(best_score DESC);

-- Create trigger to update updated_at timestamp
CREATE OR REPLACE FUNCTION update_updated_at_column()
RETURNS TRIGGER AS $$
BEGIN
    NEW.updated_at = NOW();
    RETURN NEW;
END;
$$ language 'plpgsql';

CREATE TRIGGER update_challenge_submissions_updated_at BEFORE UPDATE ON challenge_submissions
    FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();

CREATE TRIGGER update_challenge_attempts_updated_at BEFORE UPDATE ON challenge_attempts
    FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();

-- Add points earned from challenges to users table (if not exists)
ALTER TABLE users
ADD COLUMN IF NOT EXISTS challenge_points INTEGER DEFAULT 0;

-- Create view for challenge leaderboard with notebook scores
CREATE OR REPLACE VIEW challenge_leaderboard AS
SELECT 
    u.id,
    u.full_name as name,
    u.image,
    COALESCE(SUM(ca.best_score), 0) as total_score,
    COUNT(DISTINCT ca.challenge_id) as challenges_completed,
    u.points + COALESCE(SUM(ca.best_score), 0) as total_points
FROM users u
LEFT JOIN challenge_attempts ca ON u.id = ca.user_id
GROUP BY u.id, u.full_name, u.image, u.points
ORDER BY total_points DESC;

-- Grant necessary permissions (adjust as needed)
GRANT SELECT, INSERT, UPDATE ON challenge_submissions TO uj_ai_club;
GRANT SELECT, INSERT, UPDATE ON challenge_attempts TO uj_ai_club;
GRANT SELECT ON challenge_leaderboard TO uj_ai_club;
