create table playlists (
    id integer primary key not null,
    uuid text not null unique,
    name text not null,
    source_url text,
    status text not null default 'pending' check (status in ('pending', 'syncing', 'ready', 'failed')),
    prefix text,
    channel_count integer not null default 0 check (channel_count >= 0),
    last_synced_at text,
    last_error text,
    created_at text not null default (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    updated_at text not null default (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);

create index playlists_status_idx on playlists (status);

create table channel_groups (
    id integer primary key not null,
    playlist_id integer not null references playlists (id) on delete cascade on update cascade,
    name text not null,
    sort_order integer not null default 0 check (sort_order >= 0),
    created_at text not null default (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    updated_at text not null default (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    unique (playlist_id, name)
);

create index channel_groups_playlist_sort_idx on channel_groups (playlist_id, sort_order, id);

create table channels (
    id integer primary key not null,
    uuid text not null unique,
    playlist_id integer not null references playlists (id) on delete cascade on update cascade,
    group_id integer references channel_groups (id) on delete set null on update cascade,
    name text not null,
    name_override text,
    title text,
    title_override text,
    tvg_id text,
    tvg_name text,
    tvg_logo text,
    logo_override text,
    group_title text,
    group_override text,
    stream_id text,
    channel_number integer check (channel_number is null or channel_number >= 0),
    shift_seconds integer not null default 0,
    language text,
    country text,
    enabled integer not null default 1 check (enabled in (0, 1)),
    sort_order integer not null default 0 check (sort_order >= 0),
    primary_url text not null,
    created_at text not null default (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    updated_at text not null default (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);

create index channels_playlist_sort_idx on channels (playlist_id, sort_order, id);
create index channels_group_idx on channels (group_id);
create unique index channels_playlist_stream_id_unique_idx
    on channels (playlist_id, stream_id)
    where stream_id is not null;

create table channel_fallbacks (
    id integer primary key not null,
    channel_id integer not null references channels (id) on delete cascade on update cascade,
    fallback_url text not null,
    sort_order integer not null default 0 check (sort_order >= 0),
    metadata text check (metadata is null or json_valid(metadata)),
    created_at text not null default (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    updated_at text not null default (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    unique (channel_id, fallback_url)
);

create index channel_fallbacks_channel_sort_idx on channel_fallbacks (channel_id, sort_order, id);
