---
layout: page
title: About me
subtitle: Essentials
---
{% assign today = site.time | date: '%Y' %}

{% assign moved_to_isreal = 2014 %}
{% assign years_in_israel = today | minus: moved_to_isreal %}

{% assign started_working = 2008 %}
{% assign years_of_experience = today | minus: started_working %}

Hello and thank you for your interest in my blog! My name is Dmitry and I am a
professional software engineer with over {{years_of_experience }} years of experience in the
industry. I was born and grew up in Odessa, Ukraine, where I completed both my
Bachelor's and Masters's in Computer Engineering. I have been living and working in
Israel for the last {{years_in_israel}} years.
I am currently employed at GK8, a leading cyber security company specializing in blockchain security, as a Software Team Manager.
My career has spanned both global corporations, such as VMware, HP, and small-to-mid-scale startups like LiveU.
I have been blogging with increased frequency as I pinpointed my technological interests
more and more.
My summarized interests and the general categories for my blog posts can be found below:

* Multithreading, multiprocessing
* Network programming
* Low-level development
* Programming languages: C++, Rust, Python
* Design of performance-sensitive systems
* Distributed systems, consensus algorithms
* Linux toolbox and VIM fine-tuning

Please feel free to get in touch if you have a topic or question that is of interest or if you have any suggestions for future posts!
