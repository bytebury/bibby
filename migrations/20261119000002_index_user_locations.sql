-- `user_details` joins on both, and the analytics page groups users by
-- country. `city_id` is already indexed.
CREATE INDEX idx_users_country_id ON users (country_id);
CREATE INDEX idx_users_region_id ON users (region_id);
