# f-squirrel.github.io

Dmitry Danilov - Tech Blog
<https://ddanilov.me>

## Updating popular posts

The Archive page shows the top 3 most popular posts based on Google Search Console data.

1. Go to [Google Search Console](https://search.google.com/search-console) -> Performance -> Search Results
2. Set the date range to the maximum available
3. Click the **Pages** tab
4. Click **Export** -> **Download CSV**
5. Unzip and put `Pages.csv` into the `csv/` directory
6. Run:

   ```bash
   make gen-stats
   ```

   This generates `_data/popular_posts.yml` which is used by `posts.md`.
