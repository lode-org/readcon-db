```{raw} html
<div class="rc-hero">
  <div class="rc-hero-rule" aria-hidden="true"></div>
  <div class="rc-hero-brand">
    <img class="rc-hero-mark" src="_static/mark.svg" width="56" height="56" alt="" />
    <div>
      <p class="rc-hero-name"><span class="rc-hero-read">read</span><span class="rc-hero-con">con</span><span class="rc-hero-db">-db</span></p>
      <p class="rc-hero-sub">mmap CON corpus</p>
    </div>
  </div>
  <p class="rc-hero-tagline">LMDB via Heed. Indexes, xxHash3 exact match, and bindings for Rust, C, C++, Python, and Fortran.</p>
  <pre class="rc-hero-conline" aria-hidden="true">ConCorpus::open("/tmp/corpus")&#10;Select::new().require_symbol("Cu")</pre>
</div>
```

# readcon-db

Rare-event codes already checkpoint on CON. This crate keeps **thousands of
those frames** in one mmap tree: ingest CON text, filter without loading every
atom, decode a hit with [readcon-core](https://lode-org.github.io/readcon-core/).

{doc}`getting-started` · {doc}`tutorial` · {doc}`howto` · {doc}`architecture` ·
{doc}`faq`

```bash
cargo add readcon-db
pip install readcon-db
```

```{important}
*New here?* {doc}`getting-started` then {doc}`tutorial`

*Many writers / HPC shards?* {doc}`campaign`

*Language API?* {doc}`howto` · {doc}`api_rust`
```

````{grid} 1 1 2 2
:gutter: 2

```{grid-item-card} Tutorial: first corpus
:link: tutorial
:link-type: doc

Open a directory, ingest a fixture, select copper, print the hash.
```

```{grid-item-card} How-to by language
:link: howto
:link-type: doc

The same open / ingest / select path in Rust, Python, C, and Fortran.
```

```{grid-item-card} Architecture
:link: architecture
:link-type: doc

mmap, SWMR, secondary indexes, xxHash3, hourglass ABI.
```

```{grid-item-card} Campaign ops
:link: campaign
:link-type: doc

Shards, drain, join, compact. One writer per LMDB env.
```
````

## Site map

```{toctree}
:maxdepth: 1
:caption: Tutorials

getting-started
tutorial
```

```{toctree}
:maxdepth: 1
:caption: How-to guides

howto
install
campaign
workflows
```

```{toctree}
:maxdepth: 1
:caption: Explanation

faq
overview
architecture
```

```{toctree}
:maxdepth: 1
:caption: Reference

api_rust
api_c
api_python
api_fortran
```

```{toctree}
:maxdepth: 1
:caption: Project meta

contributing
changelog_link
```
