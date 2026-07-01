---
layout: page
title: Archive
---

### Most popular

{% for post in site.data.popular_posts -%}
* [{{ post.title }}]({{ post.url }})
{% endfor %}

### All posts

<div class="post">
	<ul>
	  {% for post in site.posts %}
	    <li><a href="{{ post.url }}">{{ post.title }}</a></li>
	  {% endfor %}
	</ul>
</div>
