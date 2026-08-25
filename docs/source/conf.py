project = "readcon-db"
copyright = "2026, LODE developers"
author = "LODE developers"
release = "0.1.5"
version = "0.1.5"

extensions = [
    "sphinx.ext.autodoc",
    "sphinx.ext.napoleon",
    "sphinx.ext.intersphinx",
    "sphinx_copybutton",
    "sphinx_design",
    "myst_parser",
]

templates_path = ["_templates"]
exclude_patterns = []

myst_enable_extensions = ["colon_fence", "deflist"]
source_suffix = {".rst": "restructuredtext", ".md": "markdown"}
master_doc = "index"

html_theme = "shibuya"
html_static_path = ["_static"]
html_favicon = "_static/favicon.svg"
html_title = "readcon-db documentation"
html_baseurl = "https://lode-org.github.io/readcon-db/"

html_context = {
    "source_type": "github",
    "source_user": "lode-org",
    "source_repo": "readcon-db",
    "source_version": "main",
    "source_docs_path": "/docs/source/",
}

html_sidebars = {
    "**": [
        "sidebars/localtoc.html",
        "sidebars/repo-stats.html",
        "sidebars/edit-this-page.html",
    ],
}

html_theme_options = {
    "github_url": "https://github.com/lode-org/readcon-db",
    "accent_color": "teal",
    "dark_code": True,
    "globaltoc_expand_depth": 1,
    "light_logo": "_static/logo-nav-light.svg",
    "dark_logo": "_static/logo-nav-dark.svg",
    "nav_links": [
        {"title": "Start", "url": "getting-started"},
        {
            "title": "Learn",
            "children": [
                {
                    "title": "Tutorial — first corpus",
                    "url": "tutorial",
                    "summary": "Open, ingest a fixture, select, hash",
                },
                {
                    "title": "How-to by language",
                    "url": "howto",
                    "summary": "Rust, Python, C, Fortran recipes",
                },
                {
                    "title": "Campaign ops",
                    "url": "campaign",
                    "summary": "Shards, drain, join, compact",
                },
            ],
        },
        {
            "title": "Ecosystem",
            "children": [
                {
                    "title": "readcon-core",
                    "url": "https://lode-org.github.io/readcon-core/",
                    "summary": "CON parse, write, hourglass ABI",
                },
                {
                    "title": "eOn",
                    "url": "https://eondocs.org",
                    "summary": "Saddle-point search on PESs",
                },
                {
                    "title": "rgpot",
                    "url": "https://omnipotentrpc.github.io/rgpot/",
                    "summary": "Potential evaluation toolkit",
                },
                {
                    "title": "chemparseplot",
                    "url": "https://chemparseplot.rgoswami.me",
                    "summary": "Parsing and plotting for computational chemistry",
                },
                {
                    "title": "rgpycrumbs",
                    "url": "https://rgpycrumbs.rgoswami.me",
                    "summary": "CLI helpers for LODE / eOn workflows",
                },
            ],
        },
        {"title": "Architecture", "url": "architecture"},
    ],
}

copybutton_prompt_text = r">>> |\.\.\. |\$ |In \[\d*\]: | {2,5}\.\.\.: | {5,8}: "
copybutton_prompt_is_regexp = True
copybutton_exclude = ".linenos, .gp, .go"
copybutton_line_continuation_character = "\\"
copybutton_here_doc_delimiter = "EOF"

intersphinx_mapping = {
    "python": ("https://docs.python.org/3", None),
    "readcon-core": ("https://lode-org.github.io/readcon-core/", None),
}
intersphinx_timeout = 5


def setup(app):
    app.add_css_file("custom.css")
