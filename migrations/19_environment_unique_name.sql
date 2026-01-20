-- Add unique constraint on environment name per user
ALTER TABLE environments ADD CONSTRAINT environments_user_name_unique UNIQUE (user_id, name);
