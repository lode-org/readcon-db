project('readcon-db-wrap-test', 'c', meson_version: '>=1.3.0')

readcon_dep = dependency('readcon-db', required: false)
if not readcon_dep.found()
    readcon_proj = subproject('readcon-db')
    readcon_dep = readcon_proj.get_variable('readcon_dep')
endif

executable(
    'c-main',
    'main.c',
    dependencies: readcon_dep,
)
