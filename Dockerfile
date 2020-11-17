FROM jekyll/jekyll

LABEL Description="Run ddanilov.me locally"

COPY Gemfile Gemfile
#COPY Gemfile.lock Gemfile.lock

RUN bundle install

#Weird thing but w/o it does not work
RUN gem install bundler:1.16.2
