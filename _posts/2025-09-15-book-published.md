---
title: "Back to writing: two years, one book, and a lot in between"
published: true
permalink: "/publish_book"
share-img: /img/book.jpg
tags: [book, publishing, cpp, refactoring]
readtime: true
comments: false
---

It has been more than two years since my last post, and a year since my book was published. I never wrote here about the process, but with some distance and time to reflect, I feel it’s the right moment to share the story.

The book is called ***Refactoring with C++ — Explore modern ways of developing maintainable and efficient applications***. Writing it was a long journey, both rewarding and challenging, and I’d like to tell a bit about how it came to be, how I worked on it, and what I learned along the way.

<p align="center">
  <img src="/img/book.jpg" title="Book cover">
</p>

## How It All Started

The journey began when [Packt Publishing](https://www.packtpub.com) reached out to me after my presentation at [Core C++ 2022](https://corecpp.org/) in Tel Aviv, where I gave a talk *Considerations when Working with Shared Pointers* (watch it on [YouTube](https://www.youtube.com/watch?v=hI5iBXSDbTQ)). They proposed that I write a book about refactoring in C++. After some serious consideration, I decided to take on the challenge.

## Writing Through Challenging Times

What followed was almost two years of writing. This period was marked not only by the technical and creative demands of the book itself but also by significant events unfolding around us—the war in Ukraine and the tragic events of October 7th among them. These circumstances added weight to the process, but also underscored the value of creating something constructive and enduring.

## The Joy of Structuring Thoughts

Despite the challenges, I found the writing process deeply rewarding. It gave me the rare opportunity to slow down and really think about how I approach programming. Day to day, we often jump from one task to another—bug fixes, feature requests, reviews—without pausing to ask ourselves *why* we write code in a particular way. Writing a book forced me to articulate those reasons.

I enjoyed taking practices that had become second nature to me and breaking them down into principles that others could follow. Sometimes I realized I had internalized a rule years ago but never stopped to question its origins. Other times I discovered that things I took for granted were not obvious at all, and explaining them required patience and clarity.

It was also satisfying to revisit problems I had faced earlier in my career—messy codebases, long functions, unclear ownership—and show how they can be improved. In a way, the process felt like a conversation with my younger self, but also an invitation for other developers to reflect on their own growth.

More than anything, it was rewarding to shape a narrative around ideas I had carried with me for years. Turning scattered thoughts into chapters gave me a sense of closure and clarity, and reminded me why I enjoy programming and sharing knowledge with others.

## The Writing Process

Most of the writing happened after hours and on weekends, which required discipline and persistence. After completing a draft of a chapter, I would send it to my editor, Kinnari. Since English is not my first language, my sentences were often too long, wordy, or at times unclear, and I felt sorry for the amount of work this created for her. Once the language edits were complete, the chapter moved on to the technical editor, who sometimes suggested reordering paragraphs, adding more background for less experienced readers, or even pointed out bugs in my code examples.

The process was not always linear. There were times when I would start a chapter, get stuck, put it aside, and switch to another one. Sometimes I would finish not only that chapter but also the next, and only return to the one I left behind a month later. Looking back, this rhythm actually helped: stepping away often gave me the clarity I needed to finish the difficult parts, and the feedback from editors and reviewers made it even easier to come back with fresh eyes. Each round of review not only improved the book but also helped me grow as a writer and teacher.

## The Tools I Used

Packt’s workflow relies on Microsoft Word and SharePoint, where authors upload chapters and editors add comments directly in the documents. Unfortunately, as someone who rarely works on Windows, I always found Word cumbersome—styles, paragraph settings, complex configuration, and the lack of a clear way to track changes between versions made the process difficult for me.

Instead, I decided to rely on the tools I use every day: **VS Code, the terminal, Git, and Markdown**—the same tools I use for this blog. I created a private Github repository where each chapter was written in Markdown, and I added a script using **Pandoc** to compile the drafts into PDF or DOCX files whenever I needed to preview how they would look on the page.

This workflow also allowed me to manage chapters as pull requests, which made collaboration smoother. I would upload requested changes as separate commits to the same PR, so the entire history of edits was preserved. To my delight, Kinnari was kind enough to review and comment directly on those PRs, which made the editing process feel much closer to how we work in software development.

As a little bonus, I now have a full Git log of my book, which feels like the most developer-friendly souvenir from the entire process.

<p align="center">
  <img src="/img/book-git-log.png" title="Book Git log">
</p>

## What’s Inside the Book

In ***Refactoring with C++***, I wanted to bring together the practices, principles, and tools that I believe every C++ developer should know when working with real-world codebases. Each chapter reflects not only techniques but also lessons I’ve learned from years of writing, reviewing, and maintaining C++ code.

The book begins with the basics: why clean code matters, how technical debt builds up, and how coding standards and documentation can prevent it. I then move on to the principles I rely on most in my daily work—like SOLID, abstraction, and the careful handling of mutability—and explain how these principles translate into more maintainable and understandable systems. I also take a closer look at why bad code happens in the first place, and how to spot the signs that refactoring is really needed.

From there, I go deeper into the mechanics of good C++: writing better names, making full use of the type system, designing classes and APIs thoughtfully, and recognizing patterns and anti-patterns in existing code. I tried to keep the examples practical and familiar, showing how small improvements can have a big impact on readability and maintainability. Throughout the book, I also wanted to demonstrate how modern and well-known C++ capabilities can be applied to improve existing code and help us write software that is both clear and maintainable.

The later chapters turn to tools and workflows that I personally find essential: code formatting, static and dynamic analysis, and testing at every level—from unit to acceptance. I also dedicate chapters to managing third-party libraries, using tools like Conan, vcpkg, and Docker, as well as to version control practices and code reviews, which I see as critical to building strong engineering culture.

My goal with the book was not just to provide a checklist of techniques but to share a mindset: to treat refactoring as a continuous practice that makes code clearer, safer, and more efficient.

## Support Along the Way

I had tremendous support from Packt Publishing, and in particular from my editor, [Kinnari Chohan](https://www.linkedin.com/in/kinnari-chohan/), whose guidance helped shape the raw drafts into something cohesive and polished. I am grateful as well to my former managers, [Amir Taya](https://www.linkedin.com/in/amirtaya/) and [Vladi Lyga](https://www.linkedin.com/in/vladi-lyga-732a8458/), whose perspectives on software development and code reviews I included as quotes in the book. I would like to mention [Sergey Pastukhov](https://www.linkedin.com/in/spastukhov/), from whom I learned a great deal of C++ wisdom over the years. And above all, the greatest support came from my partner, [Rina](https://www.linkedin.com/in/rina-volovich/), without whom this book would not exist.

## The Result

On July 19, 2024, *Refactoring with C++ --- Explore modern ways of developing maintainable and efficient applications* was finally published by [Packt](https://www.packtpub.com/en-us/product/refactoring-with-c-9781837633777) and is available on [Amazon](https://www.amazon.com/dp/1837633770).

If you are interested in modern C++ and want to explore refactoring techniques, practical examples, and tooling, I invite you to take a look.

## What’s Next

After taking time to reflect on the book and its journey, I want to return to sharing shorter posts—focusing on modern C++ and, more recently, on Rust, which has become a real passion of mine.

Please share your thoughts on
[LinkedIn](https://www.linkedin.com/posts/ddanilov_back-to-writing-two-years-one-book-and-activity-7373775033766162432-VZZ6) or [X](https://x.com/dbdanilov/status/1968052056775897323?s=46&t=LTvfKK96g7tvjHHSr9v4cQ).
