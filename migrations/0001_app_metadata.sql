create table app_metadata (
    key text primary key not null,
    value text not null,
    created_at text not null default (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    updated_at text not null default (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);

insert into app_metadata (key, value)
values ('initialized_at', strftime('%Y-%m-%dT%H:%M:%fZ', 'now'));
