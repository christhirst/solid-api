use solid_api::models::{
    AssignGroupMemberRequest, CreateGroupRequest, GroupRight, PlatformRole, ProvisionUserRequest,
    UpdateUserRequest,
};
use solid_api::store::Store;

#[tokio::test]
async fn test_user_provisioning_and_role_assignment() {
    let store = Store::new_in_memory()
        .await
        .expect("Failed to create in-memory store");

    // 1. Provision an admin user
    let admin_req = ProvisionUserRequest {
        sub: Some("usr_admin_01".into()),
        username: "Alice".into(),
        name: Some("Alice Admin".into()),
        email: Some("alice@example.com".into()),
        role: Some(PlatformRole::Admin),
        namespace: Some("alice-ns".into()),
    };
    let admin = store
        .provision_user(admin_req)
        .await
        .expect("Failed to provision admin");
    assert_eq!(admin.username, "alice"); // normalized
    assert_eq!(admin.sub, "usr_admin_01");
    assert_eq!(admin.idp_role, PlatformRole::Admin);
    assert_eq!(admin.name.as_deref(), Some("Alice Admin"));

    // 2. Provision a developer user (creator)
    let dev_req = ProvisionUserRequest {
        sub: None, // should auto-generate sub
        username: "Bob".into(),
        name: Some("Bob Builder".into()),
        email: Some("bob@example.com".into()),
        role: Some(PlatformRole::Creator),
        namespace: None,
    };
    let bob = store
        .provision_user(dev_req)
        .await
        .expect("Failed to provision bob");
    assert_eq!(bob.username, "bob");
    assert_eq!(bob.idp_role, PlatformRole::Creator);
    assert!(!bob.sub.is_empty());

    // 3. Duplicate username should be rejected
    let dup_req = ProvisionUserRequest {
        sub: None,
        username: "alice".into(),
        name: None,
        email: None,
        role: None,
        namespace: None,
    };
    assert!(store.provision_user(dup_req).await.is_err());

    // 4. Retrieve user by sub and by username
    let fetched_by_sub = store.get_user("usr_admin_01").await.expect("Find by sub");
    assert_eq!(fetched_by_sub.username, "alice");

    let fetched_by_username = store
        .get_user("ALICE")
        .await
        .expect("Find by username loose");
    assert_eq!(fetched_by_username.sub, "usr_admin_01");

    // 5. Update user role (assign roleOverride)
    let updated_role = store
        .update_user_role("alice", PlatformRole::Viewer, true)
        .await
        .expect("Update role override");
    assert_eq!(updated_role.role_override, Some(PlatformRole::Viewer));

    // 6. Update user profile details
    let patch = UpdateUserRequest {
        name: Some("Alice Senior Admin".into()),
        email: Some("alice.senior@example.com".into()),
        namespace: Some("alice-primary".into()),
        role: None,
        role_override: Some(None), // clear override
    };
    let updated_user = store
        .update_user("alice", patch)
        .await
        .expect("Update user");
    assert_eq!(updated_user.name.as_deref(), Some("Alice Senior Admin"));
    assert_eq!(updated_user.role_override, None);

    // 7. List users
    let all_users = store.list_users(None).await.expect("List users");
    assert_eq!(all_users.len(), 2);
}

#[tokio::test]
async fn test_group_namespaces_and_membership_rights() {
    let store = Store::new_in_memory()
        .await
        .expect("Failed to create in-memory store");

    // Provision users
    let u1 = store
        .provision_user(ProvisionUserRequest {
            sub: Some("sub_carol".into()),
            username: "carol".into(),
            name: Some("Carol".into()),
            email: Some("carol@example.com".into()),
            role: Some(PlatformRole::Creator),
            namespace: None,
        })
        .await
        .unwrap();

    let u2 = store
        .provision_user(ProvisionUserRequest {
            sub: Some("sub_dave".into()),
            username: "dave".into(),
            name: Some("Dave".into()),
            email: Some("dave@example.com".into()),
            role: Some(PlatformRole::User),
            namespace: None,
        })
        .await
        .unwrap();

    // 1. Create a root group "engineering"
    let grp1 = store
        .create_group(CreateGroupRequest {
            name: "engineering".into(),
            owner: u1.sub.clone(),
            description: Some("Engineering Organization".into()),
        })
        .await
        .expect("Create engineering group");
    assert_eq!(grp1.name, "engineering");
    assert_eq!(grp1.parent, None);

    // Creator should automatically be assigned as manager
    let members = store.list_group_members("engineering").await.unwrap();
    assert_eq!(members.len(), 1);
    assert_eq!(members[0].username, "carol");
    assert!(members[0].rights.contains(&GroupRight::Manager));

    // 2. Create hierarchical sub-group "engineering.backend"
    let sub_grp = store
        .create_group(CreateGroupRequest {
            name: "engineering.backend".into(),
            owner: u1.sub.clone(),
            description: Some("Backend Team".into()),
        })
        .await
        .expect("Create sub-group");
    assert_eq!(sub_grp.parent.as_deref(), Some("engineering"));

    // 3. Assign Dave to engineering.backend with view, use, write
    let assign_req = AssignGroupMemberRequest {
        username: u2.username.clone(),
        rights: vec![GroupRight::View, GroupRight::Use, GroupRight::Write],
        assigned_by: Some(u1.sub.clone()),
    };
    let member = store
        .assign_group_role("engineering.backend", assign_req)
        .await
        .expect("Assign dave");
    assert_eq!(member.username, "dave");
    assert_eq!(member.user_sub.as_deref(), Some("sub_dave"));
    assert_eq!(member.rights.len(), 3);

    // 4. Inspect effective access context for Dave
    let dave_access = store.get_effective_access("dave").await.unwrap();
    assert_eq!(dave_access.platform_role, PlatformRole::User);
    assert!(
        dave_access
            .group_grants
            .iter()
            .any(|g| g.contains("engineering.backend:(view,use,write)"))
    );

    // 5. Inspect effective access context for Carol (manager rights on both groups)
    let carol_access = store.get_effective_access("carol").await.unwrap();
    assert!(
        carol_access
            .group_grants
            .iter()
            .any(|g| g.contains("engineering:manager"))
    );

    // 6. Remove member
    let removed = store
        .remove_group_member("engineering.backend", "dave")
        .await
        .unwrap();
    assert!(removed);

    let members_after = store
        .list_group_members("engineering.backend")
        .await
        .unwrap();
    assert!(!members_after.iter().any(|m| m.username == "dave"));

    // 7. Delete group
    let deleted_grp = store.delete_group("engineering.backend").await.unwrap();
    assert!(deleted_grp);
}
