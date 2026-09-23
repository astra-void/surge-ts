export {};
class RoyalGuard {
  isLeader(): this is LeadGuard {
    return this instanceof LeadGuard;
  }
  isFollower(): this is FollowerGuard {
    return this instanceof FollowerGuard;
  }
}
class LeadGuard extends RoyalGuard {
  lead(): void {}
}
class FollowerGuard extends RoyalGuard {
  follow(): void {}
}

declare let royal: RoyalGuard;
if (royal.isLeader()) {
  royal.lead();
} else if (royal.isFollower()) {
  royal.follow();
}

class Recruit extends RoyalGuard {}
declare let recruit: Recruit;
if (recruit.isLeader()) {
  recruit.lead();
}

interface GuardInterface extends RoyalGuard {}
declare let viaInterface: GuardInterface;
if (viaInterface.isLeader()) {
  viaInterface.lead();
} else if (viaInterface.isFollower()) {
  viaInterface.follow();
} else {
  viaInterface.lead();
}

interface Checks {
  isLead(): this is LeadGuard;
}
declare let combined: Checks & RoyalGuard;
if (combined.isLead()) {
  combined.lead();
}

const holder = { royal };
if (holder.royal.isLeader()) {
  holder.royal.lead();
}
