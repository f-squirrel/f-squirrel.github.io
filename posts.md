---
layout: page
title: Archive
---

### Most popular in the last year
* [Implementations of std::async and how they might Affect Applications](/std-async-implementations/)
* [How to enable in-band FEC for Opus codec](/how-to-enable-in-band-fec-for-opus-codec/)
* [std::shared_ptr is an anti-pattern](/shared-ptr-is-evil/)

### All posts
<div class="post">
	<ul>
	  {% for post in site.posts %}
	    <li><a href="{{ post.url }}">{{ post.title }}</a></li>
	  {% endfor %}
	</ul>
</div>
