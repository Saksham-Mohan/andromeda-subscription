# Overview
The Subscription ADO is a smart contract designed to enable subscription-based services using blockchain technology. This ADO (Andromeda Decentralized Object) facilitates the creation, management, and utilization of subscription models by securely automating payments, duration tracking, and subscription statuses. The Subscription ADO is versatile and supports use cases such as recurring content access, memberships, or service subscriptions.

## Key Features
### Creator-Defined Subscription Models:

Content creators or service providers can define subscription offerings by specifying the duration, payment amount, and token (CW721).

### Secure Subscription Management:

Subscribers can initiate subscriptions, renew existing ones, or cancel subscriptions securely through the blockchain.

### Automatic Status Updates:

Subscriptions are automatically marked inactive upon expiration, ensuring up-to-date status tracking.

### Support for CW20 Tokens:

Payments can be made using CW20 tokens, with checks to validate authorized tokens and amounts.

### NFT-Driven Subscription Registration:

CW721 tokens can be used to register and manage unique subscription offerings, adding flexibility for NFT-based services.

## Subscription Lifecycle

### Subscription Registration:

The creator registers a subscription offering (using CW721), defining terms such as payment amount, duration, and accepted token.
Subscription Activation:

A subscriber initiates a subscription by sending the specified payment (CW20). The subscription state is created and marked as active.

### Subscription Renewal:

Subscribers can renew their subscriptions by paying the specified amount, resetting the subscription's start and end times.

### Subscription Cancellation:

Subscribers can cancel their subscriptions manually. Upon cancellation, the subscription is marked inactive, and future renewals are disabled unless reactivated.

## Conditions
The contract includes the following conditions to manage subscriptions:

### Expiration:

Subscriptions automatically become inactive once the end_time is reached.

### Payment Validation:

The exact payment amount specified in the subscription offering must be sent to activate or renew a subscription.

### Active Check:

An active subscription cannot be re-subscribed. Renewals are only allowed if the subscription is inactive.

## Queries
The following queries are available to retrieve information about subscriptions:

### Get Subscription:

Retrieve details of a specific subscription using the creator and subscriber IDs.

### List Subscriptions for Creator:

Retrieve all subscriptions associated with a specific creator.

### List Subscriptions for Subscriber:

Retrieve all subscriptions associated with a specific subscriber.

### Active Subscriptions:

Retrieve IDs of all active subscriptions.

### Subscription IDs for Creator:

Retrieve IDs of subscriptions associated with a creator.

### Subscription IDs for Subscriber:

Retrieve IDs of subscriptions associated with a subscriber.