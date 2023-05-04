# Mailing List Concept
Server with web interface with u/p so K Prefects can change it around.
Calendar feed gets published from there, not from my proton.

Simple login page then:
## look at existing events
struct containing: event,date,location,teacher,student,students associated with it,description
list of all current events

## modifiy events
list of all events includes option to modify event

## add new event
dialog to add new event

## storage
local SQL db.
web server always running.
served via an axum thing (maybe liquid templates?)

## email out events
emails evening before at 5pm

## calendar feed
use existing crate to publish ICS list, change associated students to just a number