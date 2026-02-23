# Raw backlog

## Clarifying Configuration
There is no determnistic indication of whether an event in a configuration element is a producer or consumer. However, looking at the XML and user manual, we can infer which ones are and create a sumplimentary configuration file that provides more infomration about a node.

For example

* Node: Type LCC
* Segment: Port I/O
* Group Line N

This has two event sections.

* One has the description "(C) When this event occurs"
* One has "(P) this event will be sent"

That provides very clear information we can use to determine if it's a producer or consumer.

### AI Classification

We can use AI to create these documents that provide more details.