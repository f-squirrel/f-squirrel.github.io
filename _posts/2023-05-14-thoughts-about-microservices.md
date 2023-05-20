---
title: My thoughts on microservices and monoliths
published: true
permalink: "/thoughts-about-microservices"
tags: [design, architecture, microservices]
readtime: true
comments: false
---

Recently, several friends directed my attention to an intriguing [article][1] detailing Amazon Prime's transition from a microservices architecture back to a monolith.
It seems like this change of paradigm has triggered a lot of debates in the network, so I decided to share my thoughts on the topic.

The dawn of microservices was a response to the mounting challenges faced by traditional monolithic architectures. As businesses began to expand at an unprecedented pace, the scalability limitations of these monolithic systems became increasingly apparent. At this critical juncture, Amazon emerged as a pioneering force, demonstrating the potential of a new architectural approach: microservices.

In the Amazon paradigm, software applications were no longer viewed as unwieldy monoliths, but as collections of loosely coupled, independently deployable services. Each microservice served a specific function and could be developed, deployed, and scaled independently. This architectural shift offered a level of flexibility and resilience that was previously unattainable, particularly when it came to scalability.

Impressed by the advantages demonstrated by Amazon, many Internet companies quickly followed suit. They recognized that this new approach could help them better manage complexity, improve system resilience, and speed up deployment times. The era of microservices had begun, setting the stage for a transformative period in the realm of software development.

Since microservices became popular, as an engineer, I was constantly seeing projects that would separate their apps into microservices just for the sake of it, extracting every little functionality into an additional service, or lambda. This was supposed to magically solve all their technical issues: poor understanding of the problem they were trying to solve, bad design or incompetent developers. Moreover, the lack of microservices would be an excuse for why a million users did not use the product every single second. And Amazon, as the biggest cloud provider, would be only happy to support their decision to adopt the new architecture, with proprietary message brokers, storage services, and tracing implementations preventing the clients from switching to other providers.

Microservices promoted at least one new job title: DevOps.
Originally DevOps had nothing to do with the cloud, but who remembers that? Eventually, every little company needed a DevOps, software architect, a solution architect, and a few other architects. Otherwise, who would remember how all your thousand microservices talk to each other or how to deploy this monstrosity?

Before the whole industry jumps back to monolith just because "microservices are not cool anymore" or "FAANG use monolith", I would like to look closer at the Amazon Prime case.

The first bottleneck was related to orchestration management, which I am not an expert in. But it sounds a bit like an issue of AWS itself to me.
However, the second one looked more interesting. Let me quote here:

> The second cost problem we discovered was about the way we were passing video frames (images) around different components. To reduce computationally expensive video conversion jobs, we built a microservice that splits videos into frames and temporarily uploads images to an Amazon Simple Storage Service (Amazon S3) bucket. Defect detectors (where each of them also runs as a separate microservice) then download images and processed it concurrently using AWS Lambda. However, the high number of Tier-1 calls to the S3 bucket was expensive.

I have never worked in Amazon but I do have some experience in the video and audio industry. It sounds obvious to me that a video processing system requires a quick communication channel able to transfer big video frames, especially if the processing has anything to do with near-real time. Since the processing most probably happens on a GPU, the time spent on it becomes small relative to the time spent on transfer. Just imagine, that the sender has to copy the frame from user space to kernel space, send it over the network (friends, it is expensive even inside the same region or whatever), and do the same in the opposite order at the receiver side. Oh, and save and flush to the quickest file system S3, but still a filesystem. And resend it again. I am surprised they hit the problem only now. The solution is groundbreaking: to avoid expensive data transfer, it is implemented within the same process.

Ironically, very soon after migrating to the monolith, Amazon engineers realized that the vertical scale was not good enough and had to run multiple instances of the new monolith to extend capacity. Additionally, they have had to implement a lightweight orchestration to manage them. I wonder if the new orchestration may become a new bottleneck.

While this shift in architecture seems justified, I would like to mention a few things in defense of microservices.

Microservices do help companies to decouple components and, what is more important, teams working on them. It is a very satisfying thought that the bug you might be creating right now will be isolated in this very component. The overhead of adding a new API endpoint often leads people to think twice before designing it, the internal communication framework makes interfaces more formal, and uniform, with clear APIs. It leaves less room for dangerous corner-cutting.
No crazy people calling your code outside of the event-dispatch loop only because the language allows it. The separation of microservices allows choosing a programming language per component. Yes, if your developers advocate RDD (resume driven development) you might end up with dozens of languages. Nevertheless, it is sad to see C++ code struggling with I/O and Python with threads only because the project is a monolith process implemented in a single language.

According to Amazon's article, the new architecture reuses the same components as before. It means the merge of microservices into a single instance was easy, probably because of the clear interfaces of the original design.

If the system is I/O bound (most of the web services, data proxies, and storage systems) or even-driven, network-based APIs are the simplest way to achieve asynchronicity.
Most languages provide async/await support that offloads the complexity of implementation to the kernel and userspace async schedulers.
The only alternative I am familiar with is thread-based: with producer-consumer queues in the best case or direct usage of mutexes in the worst. It never ends well, though. Multithreading is too complicated.

I am a big fan of unit testing, it allows developers to ensure the behavior of every component, and to mock certain classes or functions, but in many cases, it is often not sufficient. It is often hard to test the interaction between bigger components. It might be done via end-to-end tests which are easier to create for isolated components.

In the end, I would like to remind that changing architecture is a dramatic milestone in a product's life.
For the last 30 years, the industry moved from structured programming to object-oriented, and then to functional, from compiled languages to dynamic ones. All these technologies are good but none of them was a silver bullet.
So if the whole industry shifts to a new direction, it does not mean that it will fit your needs.
In conclusion, if you've identified that microservices are at the heart of your project's difficulties, here are some points to ponder before deciding to overhaul your system completely to a monolithic architecture.

1. If your service is primarily I/O bound, a switch may not be the best course of action. It's likely that a monolithic architecture won't provide the solution to your problems.
2. if you're experiencing slow network communication, consider consolidating multiple services on the same instance. This way, communication will occur through the loopback, potentially improving speed and efficiency.
3. If consolidating multiple services does not sufficiently speed up communication, another strategy is to create a drop-in replacement for the existing network API-based communication framework. This substitute should facilitate asynchronous in-process communication. To further improve efficiency, relocate the services that are most tightly interlinked into a single process and utilize the newly-developed framework to establish connections between them.

Please share your thoughts on [Twitter][2], or [LinkedIn][3].

[1]: https://www.primevideotech.com/video-streaming/scaling-up-the-prime-video-audio-video-monitoring-service-and-reducing-costs-by-90
[2]: https://twitter.com/dbdanilov/status/1658194978852462592?s=46&t=LTvfKK96g7tvjHHSr9v4cQ
[3]: https://www.linkedin.com/posts/ddanilov_my-thoughts-on-microservices-and-monoliths-activity-7063960143113199616-4wR_