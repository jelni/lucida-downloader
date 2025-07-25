# lucida-downloader

a multithreaded client for downloading music for free with
[lucida](https://lucida.to/).

<a href="https://brainmade.org/">
  <picture>
    <source media="(prefers-color-scheme: dark)" srcset="https://brainmade.org/white-logo.svg">
    <img alt="Brainmade mark" src="https://brainmade.org/black-logo.svg">
  </picture>
</a>

## installation

```
cargo install --git https://github.com/jelni/lucida-downloader
```

## usage

- find the albums you want to download on https://play.qobuz.com/ (requires an
  account, but provides superior experience) or https://www.qobuz.com/shop

- run
  ```
  lucida <urls>
  ```

```
Usage: lucida [OPTIONS] [URLS]...

Arguments:
  [URLS]...  URLs to download

Options:
  -f, --file <FILE>                    files to read URLs from
  -o, --output <OUTPUT>                custom path to download to
      --album-year <ALBUM_YEAR>        use "<album> (year)" or "(year) <album>" directory name [possible values: append, prepend]
      --flatten-directories            use "<artist> - <album>" format instead of nested "<artist>/<album>" directories
      --country <COUNTRY>              country to use accounts from [default: auto]
      --no-metadata                    disable metadata embedding by lucida
      --private                        hide tracks from recent downloads on lucida
      --album-workers <ALBUM_WORKERS>  amount of albums to download simultaneously [default: 1]
      --track-workers <TRACK_WORKERS>  amount of tracks to download simultaneously for each album [default: 4]
      --skip-tracks                    skip downloading tracks in the album
      --skip-cover                     skip downloading album cover
  -h, --help                           Print help
```

> [!NOTE]  
> remember to support your favorite artists!
